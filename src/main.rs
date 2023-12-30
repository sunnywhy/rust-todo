use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{delete, get, post, put};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect("postgres://johndoe:randompassword@localhost:5432/todolist")
        .await
        .unwrap();

    let app = Router::new()
        .route("/", get(|| async { "Hello, World!" }))
        .route("/todos", get(get_todos))
        .route("/todos", post(add_todo))
        .route("/todos/:id", get(get_todo))
        .route("/todos/:id", put(update_todo))
        .route("/todos/:id", delete(delete_todo));
    let app = app
        .layer(TraceLayer::new_for_http())
        .fallback(handler_404)
        .with_state(pool);

    let listener = TcpListener::bind("127.0.0.1:3000").await.unwrap();
    tracing::debug!("Listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

#[derive(Debug, Serialize, Clone)]
struct Todo {
    id: i32,
    description: String,
    completed: bool,
}

// The query parameters for todos list
#[derive(Debug, Deserialize, Default)]
pub struct ListOptions {
    pub offset: usize,
    pub limit: usize,
}

async fn get_todos(
    State(pool): State<PgPool>,
    options: Query<ListOptions>,
) -> Result<Json<Vec<Todo>>, (StatusCode, String)> {
    let todos = sqlx::query_as!(
        Todo,
        r#"
        SELECT id, description, completed
        FROM todos
        ORDER BY id
        OFFSET $1
        LIMIT $2
        "#,
        options.offset as i64,
        options.limit as i64
    )
    .fetch_all(&pool)
    .await
    .map_err(internal_error)?;

    Ok(Json(todos))
}

#[derive(Debug, Deserialize)]
struct CreateTodo {
    description: String,
}

async fn add_todo(
    State(pool): State<PgPool>,
    Json(input): Json<CreateTodo>,
) -> Result<Json<Todo>, (StatusCode, String)> {
    let todo = sqlx::query_as!(
        Todo,
        r#"
        INSERT INTO todos (description, completed)
        VALUES ($1, $2)
        RETURNING id, description, completed
        "#,
        input.description,
        false
    )
    .fetch_one(&pool)
    .await
    .map_err(internal_error)?;

    Ok(Json(todo))
}

async fn get_todo(
    State(pool): State<PgPool>,
    Path(id): Path<i32>,
) -> Result<Json<Todo>, (StatusCode, String)> {
    let todo = sqlx::query_as!(
        Todo,
        r#"
        SELECT id, description, completed
        FROM todos
        WHERE id = $1
        "#,
        id
    )
    .fetch_one(&pool)
    .await
    .map_err(internal_error)?;

    Ok(Json(todo))
}

#[derive(Debug, Deserialize)]
struct UpdateTodo {
    description: Option<String>,
    completed: Option<bool>,
}

async fn update_todo(
    State(pool): State<PgPool>,
    Path(id): Path<i32>,
    Json(update_todo): Json<UpdateTodo>,
) -> Result<Json<Todo>, (StatusCode, String)> {
    let todo = sqlx::query_as!(
        Todo,
        r#"
        UPDATE todos
        SET description = $1, completed = $2
        WHERE id = $3
        RETURNING id, description, completed
        "#,
        update_todo.description.unwrap_or("".to_string()),
        update_todo.completed.unwrap_or(false),
        id
    )
    .fetch_one(&pool)
    .await
    .map_err(internal_error)?;

    Ok(Json(todo))
}

async fn delete_todo(
    State(pool): State<PgPool>,
    Path(id): Path<i32>,
) -> Result<Json<Todo>, (StatusCode, String)> {
    let todo = sqlx::query_as!(
        Todo,
        r#"
        DELETE FROM todos
        WHERE id = $1
        RETURNING id, description, completed
        "#,
        id
    )
    .fetch_one(&pool)
    .await
    .map_err(internal_error)?;

    Ok(Json(todo))
}

async fn handler_404() -> impl IntoResponse {
    (StatusCode::NOT_FOUND, "Not Found")
}

fn internal_error<E>(error: E) -> (StatusCode, String)
where
    E: std::error::Error,
{
    tracing::error!("Unhandled error: {:?}", error);
    (StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
}
