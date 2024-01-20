// TODO
#[utoipa::path(
    get, 
    path = "/api/request_chat_id", 
    tag = "task",
    security(
        ("api_key" = ["edit:items", "read:items"])
))]
pub async fn request_chat_id() -> String {
    String::from("some-id")
}
