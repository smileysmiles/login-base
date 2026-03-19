# login-base

Small Rust login service built with Axum. The current implementation is intentionally simple and keeps the application, HTTP, domain, and infrastructure concerns separated.

## Structure

- `src/domain`: domain types used by the use case
- `src/app`: login use case and application-owned ports
- `src/http`: HTTP DTOs and Axum route wiring
- `src/infra`: in-memory and mock adapters used by the executable
- `docs`: PlantUML diagrams for architecture, deployment, and flow coverage

## Current Login Flow

The executable in [src/main.rs](C:\Dev\login-base\src\main.rs#L1) wires:

- `InMemoryAuthAccountRepository` seeded with one demo user
- `MockComplianceService`, which always returns `false`
- `LoginService`, exposed to HTTP as `Arc<dyn LoginUseCase + Send + Sync>`

The login flow in [src/app/login/service.rs](C:\Dev\login-base\src\app\login\service.rs#L1) is:

1. Load the auth account by username.
2. If no auth account is found, return `LoginError::InvalidCredentials`.
3. If the account is locked, return `LoginError::AccountLocked`.
4. Compare the supplied password with the stored `password_hash` using plain-text equality.
5. If the password does not match, return `LoginError::InvalidCredentials`.
6. Run the compliance check.
7. If excluded, return `LoginError::SelfExcluded`.
8. Otherwise return success with message `"OK"`.

Internal outcomes are explicit, but the HTTP layer deliberately collapses all failures into the same external response to avoid exposing account state.

## HTTP API

The router is defined in [src/http/routes.rs](C:\Dev\login-base\src\http\routes.rs#L1) and currently exposes:

- `GET /health`
- `POST /login`

Request DTO in [src/http/dto.rs](C:\Dev\login-base\src\http\dto.rs#L1):

```json
{
  "username": "demo",
  "password": "password"
}
```

Success response:

```json
{
  "status": "ok",
  "message": "OK"
}
```

Failure response:

```json
{
  "status": "error",
  "message": "Authentication failed"
}
```

Current HTTP behavior:

- health returns `200 OK` with `{"status":"ok"}`
- success returns `200 OK`
- all authentication failures return `401 Unauthorized`
- the response body does not distinguish between unknown user, wrong password, locked account, or self-excluded account

## Demo Data

The current in-memory repository contains a single demo auth account:

- username: `demo`
- password: `password`
- locked: `false`

This is only suitable for local development. The repository is not shared across instances and is reset on restart.

## Tests

The current automated coverage is focused on the login use case and the HTTP boundary.

Application tests in [src/app/login/service.rs](C:\Dev\login-base\src\app\login\service.rs#L44) cover:

- valid login
- unknown username
- wrong password
- locked account
- self-excluded account

HTTP tests in [src/http/routes.rs](C:\Dev\login-base\src\http\routes.rs#L54) cover:

- success response shape and `200` status
- generic failure response shape and `401` status
- the non-enumerating behavior for all failure branches

Run the test suite with:

```bash
cargo test
```

## Perf Checks

Local perf checks are treated as regression indicators, not production benchmarks. The current setup uses a fixed `k6` scenario against the in-memory/mock application and stores a reduced summary for comparison over time.

Files:

- `perf/login.js`: fixed load scenario for `POST /login`
- `perf/run.ps1`: runs `k6`, stores a summary in `perf/results`, and compares to `perf/baseline.json`
- `perf/promote-baseline.ps1`: promotes the latest or a chosen result into `perf/baseline.json`
- `perf/baseline.json`: baseline metrics and warn/fail guardrails

Default scenario:

- `20` virtual users
- `30s` duration
- successful login payload for the seeded demo auth account

Captured metrics:

- `p50`, `p95`, `p99`
- average latency
- throughput
- failure rate

Run it with the service already running locally:

```powershell
.\perf\run.ps1
```

Promote the latest result into the baseline:

```powershell
.\perf\promote-baseline.ps1
```

Current guardrails in `perf/baseline.json`:

- warn on `10%` regression for `p95`, `p99`, `avg`, or throughput drop
- fail on `20%` regression for `p95`, `p99`, `avg`, or throughput drop
- warn if failure rate is at least `1%`
- fail if failure rate is at least `5%`

The script exits with:

- `0` for pass
- `1` for warn
- `2` for fail

`p50` is still stored for visibility, but guardrails currently focus on `p95`, `p99`, `avg`, throughput, and failure rate because they are more stable and useful for this local in-memory setup.

## Notes

- Password checking is currently plain-text comparison.
- Compliance is currently mocked.
- CORS currently allows `http://localhost:5173`.
