pub struct DatabaseConnection {
}

impl DatabaseConnection {
  pub async fn get_user_by_id(&self, id: &i32) {
    println!("get_user_by_id handling_user={id}");
  }
}



