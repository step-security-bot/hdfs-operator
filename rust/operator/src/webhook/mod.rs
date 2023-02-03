use futures::pin_mut;

mod webhook;
mod webhook_cert_manager;

pub async fn start(client: &stackable_operator::client::Client) {
    let webhook = webhook::start(ctx);
    pin_mut!(webhook);
    webhook.await;
}
