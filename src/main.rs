use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use actix_web::web::Data;
use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::json;
use tokio::sync::RwLock;

struct State {
    db: AppState,
}

type AppState = Arc<HashMap<u8, RwLock<Account>>>;

#[derive(Default, Clone)]
struct Account {
    balance: i64,
    limit: i64,
    transactions: RingBuffer<Transaction>,
}

impl Account {
    fn with_limit(limit: i64) -> Self {
        Self {
            balance: 0,
            limit,
            transactions: Default::default(),
        }
    }
}

#[derive(Clone, Serialize)]
struct RingBuffer<T>(VecDeque<T>);

impl<T> Default for RingBuffer<T> {
    fn default() -> Self {
        Self::with_capacity(10)
    }
}

impl<T> RingBuffer<T> {
    pub fn with_capacity(capacity: usize) -> Self {
        Self(VecDeque::with_capacity(capacity))
    }

    pub fn push(&mut self, item: T) {
        if self.0.len() == self.0.capacity() {
            self.0.pop_back();
            self.0.push_front(item);
        } else {
            self.0.push_front(item);
        }
    }
}

#[derive(Deserialize, Serialize, Clone, Debug)]
enum TransactionType {
    #[serde(rename = "c")]
    Credit,
    #[serde(rename = "d")]
    Debit,
    #[serde(other)]
    None,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
struct Transaction {
    #[serde(rename = "valor")]
    pub value: i64,
    #[serde(rename = "tipo")]
    pub transaction_type: String,
    #[serde(rename = "descricao", deserialize_with = "deserialize_null_default")]
    pub description: String,
}

fn deserialize_null_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
    where
        T: Default + Deserialize<'de>,
        D: Deserializer<'de>,
{
    let opt = Option::deserialize(deserializer)?;
    Ok(opt.unwrap_or_default())
}

#[post("/{id}/transacoes")]
async fn transactions(
    req_body: web::Json<Transaction>,
    id: web::Path<u8>,
    app_state: Data<State>,
) -> impl Responder {
    let client_id = id.into_inner();
    let payload = req_body.into_inner();

    match validate_payload(&payload) {
        Some(response) => return response,
        None => (),
    }

    match app_state.db.get(&client_id) {
        Some(account) => {
            let mut account = account.write().await;
            match payload.transaction_type.as_ref() {
                "c" => {
                    account.balance += payload.value;
                    account.transactions.push(payload);
                    return HttpResponse::Ok().json(json!({
                        "limite": account.limit,
                        "saldo": account.balance,
                    }));
                }
                "d" => {
                    if account.balance + account.limit >= payload.value {
                        account.balance -= payload.value;
                        account.transactions.push(payload);
                        return HttpResponse::Ok().json(json!({
                            "limite": account.limit,
                            "saldo": account.balance,
                        }));
                    } else {
                        HttpResponse::UnprocessableEntity().finish()
                    }
                }
                _ => HttpResponse::UnprocessableEntity().finish(),
            }
        }
        None => HttpResponse::NotFound().finish(),
    }
}

#[get("/{id}/extrato")]
async fn statements(id: web::Path<u8>, app_state: Data<State>) -> impl Responder {
    let client_id = id.into_inner();
    match app_state.db.get(&client_id) {
        Some(account) => {
            let account = account.read().await;

            HttpResponse::Ok().json(json!({
                "saldo": {
                    "total": account.balance,
                    "limite": account.limit,
                    "data_extrato": "2024-01-17T02:34:41.217753Z",
                },
                "ultimas_transacoes": account.transactions
            }))
        }
        None => HttpResponse::NotFound().finish(),
    }
}

fn validate_payload(req_body: &Transaction) -> Option<HttpResponse> {
    if req_body.description.len() < 1 || req_body.description.len() > 10 {
        return Some(HttpResponse::UnprocessableEntity().finish());
    }

    if req_body.transaction_type.is_empty() && "c" != req_body.transaction_type.as_str() && "d" != req_body.transaction_type.as_str() {
        return Some(HttpResponse::UnprocessableEntity().finish());
    }

    return None;
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new()
            .app_data(Data::new(State {
                db: HashMap::<u8, RwLock<Account>>::from_iter([
                    (1, RwLock::new(Account::with_limit(100_000))),
                    (2, RwLock::new(Account::with_limit(80_000))),
                    (3, RwLock::new(Account::with_limit(1_000_000))),
                    (4, RwLock::new(Account::with_limit(10_000_000))),
                    (5, RwLock::new(Account::with_limit(500_000))),
                ])
                    .into(),
            }))
            .service(
                web::scope("/clientes")
                    .service(transactions)
                    .service(statements),
            )
    })
        .bind(("0.0.0.0", 3000))?
        .run()
        .await
}
