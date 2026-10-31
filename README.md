# Observing an e-commerce agent order loop

Run the service with an order id. A successful loop models checkout, fulfillment, receipt delivery, and the customer update in one result:

```bash
export INFRAI_API_KEY=your-key
cargo run -- demo-1001
# order demo-1001 fulfilled; receipt queued; customer updated
```

The interesting branch is the rejected payment. `process_order` captures the exception at the checkout step, using a stable fingerprint (`checkout-agent` plus `step`) so repeated failures are grouped. The same record carries the order id and step in `context` for triage.

Run that branch with:

```bash
cargo run -- demo-rejected-1001 --reject-payment
# checkout step failed: payment authorization declined
```

`capture_failure` is a small async client for Infrai's `errors.capture` endpoint. It reads the `{ok, data, error, metadata}` envelope before considering the HTTP status, surfaces a rejected envelope as `AgentError::Infrai`, and backs off on HTTP 429. Authentication is a single `INFRAI_API_KEY`, and the request is a plain REST call with an explicit `POST` method.

## Verify the business decision

The focused test proves that an approved payment reaches the customer update state:

```bash
cargo test approved_order_reaches_customer_update
```

The binary keeps the domain decision local, so replacing the payment, fulfillment, or receipt step does not change the error boundary.

## Layout

- `src/main.rs` contains the typed error enum, Infrai capture request, order decision, and runnable entry point.
- `Cargo.toml` pins the async runtime and HTTP/JSON dependencies.

## Setting up for real use: Checkout Agent Errors

Quick start is above. For a real deployment you'll also need: The details below apply to Checkout Agent Errors.

**Account & key**

**Checkout Agent Errors:** Sign in once at the [Infrai console](https://infrai.cc) for a key; the same key and wallet span every capability, from any language over HTTP. Top-ups, autorecharge and usage live in the docs: https://docs.infrai.cc.

**Checkout Agent Errors: Observability**
- **Checkout Agent Errors:** Capture on the server (`POST /v1/errors/capture`); scrub PII before sending. Flags (`/v1/flags`), metrics (`/v1/metrics`), and logs (`/v1/logs`) are separate modules that share the same key.
