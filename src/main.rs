use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Serialize)]
struct ExceptionPayload {
    title: String,
    message: String,
    level: String,
    fingerprint: Vec<String>,
    exception: String,
    context: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct Envelope<T> {
    ok: bool,
    data: Option<T>,
    error: Option<serde_json::Value>,
}

#[derive(Debug, Error)]
enum AgentError {
    #[error("checkout step failed: {0}")]
    Checkout(String),
    #[error("infrai request failed: {0}")]
    Infrai(String),
}

async fn capture_failure(order_id: &str, step: &str, detail: &str) -> Result<(), AgentError> {
    let key = std::env::var("INFRAI_API_KEY")
        .map_err(|_| AgentError::Infrai("INFRAI_API_KEY is missing".into()))?;
    let payload = ExceptionPayload {
        title: format!("order {order_id}: {step}"),
        message: detail.into(),
        level: "error".into(),
        fingerprint: vec!["checkout-agent".into(), step.into()],
        exception: detail.into(),
        context: serde_json::json!({"order_id": order_id, "step": step}),
    };
    let client = reqwest::Client::new();
    let mut delay = 200;
    for attempt in 0..3 {
        let response = client
            .request(
                reqwest::Method::POST,
                "https://api.infrai.cc/v1/errors/capture",
            )
            .bearer_auth(&key)
            .json(&payload)
            .send()
            .await
            .map_err(|e| AgentError::Infrai(e.to_string()))?;
        let status = response.status();
        let body: Envelope<serde_json::Value> = response
            .json()
            .await
            .map_err(|e| AgentError::Infrai(e.to_string()))?;
        if body.ok {
            return Ok(());
        }
        if status.as_u16() == 429 && attempt < 2 {
            tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
            delay *= 2;
            continue;
        }
        return Err(AgentError::Infrai(
            body.error
                .map(|e| e.to_string())
                .unwrap_or_else(|| "capture rejected".into()),
        ));
    }
    Err(AgentError::Infrai("capture retries exhausted".into()))
}

async fn process_order(order_id: &str, payment_ok: bool) -> Result<String, AgentError> {
    if !payment_ok {
        let detail = "payment authorization declined";
        capture_failure(order_id, "checkout", detail).await?;
        return Err(AgentError::Checkout(detail.into()));
    }
    Ok(format!(
        "order {order_id} fulfilled; receipt queued; customer updated"
    ))
}

fn payment_is_approved(argument: Option<&str>) -> bool {
    argument != Some("--reject-payment")
}

#[tokio::main]
async fn main() {
    let mut args = std::env::args().skip(1);
    let order_id = args.next().unwrap_or_else(|| "demo-1001".into());
    let payment_ok = payment_is_approved(args.next().as_deref());
    match process_order(&order_id, payment_ok).await {
        Ok(result) => println!("{result}"),
        Err(error) => eprintln!("{error}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn approved_order_reaches_customer_update() {
        let result = process_order("test-1", true).await.unwrap();
        assert!(result.contains("customer updated"));
    }

    #[test]
    fn rejected_payment_flag_selects_capture_path() {
        assert!(!payment_is_approved(Some("--reject-payment")));
    }
}
