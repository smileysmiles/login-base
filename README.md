# login-base

Small Rust login service built with Axum. The current implementation is intentionally simple and keeps the application, HTTP, domain, and infrastructure concerns separated.

## Structure

- `src/domain`: domain types used by the use case
- `src/app`: login use case and application-owned ports
- `src/http`: HTTP DTOs and Axum route wiring
- `src/infra`: in-memory and mock adapters used by the executable
- `docs`: PlantUML diagrams for architecture, deployment, and flow coverage

## Service Boundary

This service is intended to stay cohesive around authentication and session concerns:

- login and logout
- token/session validation
- password-oriented auth flows such as change/reset
- minimal authenticated-subject lookup through `GET /me`

It is not intended to become the source of broader player or account profile data. Richer player information should live behind a separate player-facing service. In the current PoC, `GET /me` exists mainly to prove to the UI that authentication really succeeded and that the current token still maps to an active authenticated session.

The practical boundary is:

- auth service returns only minimal identity needed to confirm the authenticated subject
- player/profile data belongs to a separate downstream service
- `GET /me` may later become a thin facade over another service, but it should stay deliberately narrow

## Current Login Flow

The executable in [src/main.rs](C:\Dev\login-base\src\main.rs#L1) wires:

- `InMemoryAuthAccountRepository` seeded with one demo user
- `InMemorySessionStore` for local session records
- `MockComplianceService`, which always returns `false`
- `JwtTokenIssuer`, which creates a session and signs a JWT with `sid`
- `JwtSessionManager`, which validates JWTs against active session state
- `LoginService`, exposed to HTTP as `Arc<dyn LoginUseCase + Send + Sync>`
- `ChangePasswordService`
- `PasswordResetService`

The login flow is split between [src/app/login/authenticate.rs](C:\Dev\login-base\src\app\login\authenticate.rs#L1) and [src/app/login/service.rs](C:\Dev\login-base\src\app\login\service.rs#L1):

1. Load the auth account by username.
2. If no auth account is found, return `LoginError::InvalidCredentials`.
3. If the account is locked, return `LoginError::AccountLocked`.
4. Compare the supplied password with the stored `password_hash` using plain-text equality.
5. If the password does not match, increment failed attempts and lock the account on the third consecutive failure.
6. Run the compliance check.
7. If excluded, return `LoginError::SelfExcluded`.
8. Otherwise clear failed attempts, create an in-memory session, and issue a JWT for the authenticated account.
9. Return success with message `"OK"`, the token, and minimal subject identity.

Internal outcomes are explicit, but the HTTP layer deliberately collapses all failures into the same external response to avoid exposing account state.

## Observability Port

The auth boundary now emits typed business/security events through an application-owned `Observability` port (`src/app/ports/observability.rs`) and wires a local adapter (`src/infra/mock_observability.rs`) in the executable.

Current emitted events include:

- login success/failure and lockout
- password change success/failure
- password reset request/completion/failure
- logout success/failure
- `/me` lookup success/failure

The executable now supports two adapters selected by `LOGIN_BASE_OBSERVABILITY`:

- `telemetry` (default): structured JSON logs + trace spans + monotonic event counters via `tracing`
- `mock`: simple `auth_event={...}` stdout lines
- `none`: disables business-event emission (useful for performance isolation runs)

`telemetry` mode is intended as the production-oriented default and keeps sensitive values out of events (no passwords or raw tokens).

### OTLP Export

When running in `telemetry` mode, trace export is enabled by setting:

- `OTEL_EXPORTER_OTLP_ENDPOINT` (for example `http://localhost:4317`)
- optional `OTEL_SERVICE_NAME` (defaults to `login-base`)
- optional `OTEL_EXPORTER_OTLP_HEADERS` for vendor/API auth headers
- optional sampler controls:
  - `LOGIN_BASE_TRACE_SAMPLER` (or `OTEL_TRACES_SAMPLER`)
  - `LOGIN_BASE_TRACE_SAMPLER_RATIO` (or `OTEL_TRACES_SAMPLER_ARG`)

Supported sampler values:

- `always_on`
- `always_off`
- `traceidratio`
- `parentbased_always_on`
- `parentbased_always_off`
- `parentbased_traceidratio` (default)

Default sampling ratio is `0.05` (5% traces) when a ratio-based sampler is selected.

If `OTEL_EXPORTER_OTLP_ENDPOINT` is not set, telemetry stays local (structured JSON logs only).

Telemetry mode also adds request-level tracing middleware:

- generates or reuses `x-request-id`
- propagates `x-request-id` on responses
- emits `http.request` spans with `method`, `path`, and `request_id`
- emits request completion/failure logs with `status` and `latency_ms`

Set `LOGIN_BASE_HTTP_TRACE_ENABLED=false` to disable HTTP request trace/log middleware for perf-focused runs.

#### Local Collector Quickstart

1. Start a local OpenTelemetry Collector using the bundled config:

```powershell
docker run --rm -p 4317:4317 --mount type=bind,source="C:\Dev\login-base\docs\otel-collector-config.yaml",target=/etc/otelcol/config.yaml,readonly otel/opentelemetry-collector:latest
```

2. In a second terminal, run the app with telemetry + OTLP:

```powershell
$env:LOGIN_BASE_OBSERVABILITY="telemetry"
$env:OTEL_EXPORTER_OTLP_ENDPOINT="http://localhost:4317"
$env:OTEL_SERVICE_NAME="login-base"
cargo run
```

3. Exercise an endpoint (for example `POST /login`) and confirm spans appear in collector output.

### Run Config Scenarios

Use these CMD examples as copy/paste profiles.

1. Local dev, business events only (simple stdout):

```cmd
set "LOGIN_BASE_OBSERVABILITY=mock"
set "LOGIN_BASE_HTTP_TRACE_ENABLED=true"
cargo run
```

2. Telemetry logs/traces locally (no OTLP export):

```cmd
set "LOGIN_BASE_OBSERVABILITY=telemetry"
set "LOGIN_BASE_HTTP_TRACE_ENABLED=true"
set "OTEL_EXPORTER_OTLP_ENDPOINT="
cargo run
```

3. Telemetry + OTLP, low-overhead sampled tracing (recommended default):

```cmd
set "LOGIN_BASE_OBSERVABILITY=telemetry"
set "LOGIN_BASE_HTTP_TRACE_ENABLED=true"
set "OTEL_EXPORTER_OTLP_ENDPOINT=http://127.0.0.1:4317"
set "OTEL_SERVICE_NAME=login-base"
set "LOGIN_BASE_TRACE_SAMPLER=parentbased_traceidratio"
set "LOGIN_BASE_TRACE_SAMPLER_RATIO=0.05"
cargo run
```

4. Telemetry + OTLP with full tracing (debug only, expensive):

```cmd
set "LOGIN_BASE_OBSERVABILITY=telemetry"
set "LOGIN_BASE_HTTP_TRACE_ENABLED=true"
set "OTEL_EXPORTER_OTLP_ENDPOINT=http://127.0.0.1:4317"
set "LOGIN_BASE_TRACE_SAMPLER=always_on"
cargo run
```

5. Perf isolation mode (disable observability overhead):

```cmd
set "LOGIN_BASE_OBSERVABILITY=none"
set "LOGIN_BASE_HTTP_TRACE_ENABLED=false"
cargo run
```

6. Perf run command (separate terminal, after starting app):

```powershell
.\perf\run.ps1 -Scenario login
.\perf\run.ps1 -Scenario login-failure
```

## HTTP API

The router is defined in [src/http/routes.rs](C:\Dev\login-base\src\http\routes.rs#L1) and currently exposes:

- `GET /health`
- `POST /login`
- `POST /change-password`
- `POST /forgot-password`
- `POST /reset-password`
- `GET /me`
- `POST /logout`

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
  "account_id": 1,
  "username": "demo",
  "message": "OK",
  "token": "<jwt>"
}
```

`GET /me` success response:

```json
{
  "account_id": 1,
  "username": "demo"
}
```

`POST /change-password` request:

```json
{
  "username": "demo",
  "current_password": "password",
  "new_password": "new-password"
}
```

`POST /change-password` success response:

```json
{
  "status": "ok",
  "message": "Password changed"
}
```

`POST /forgot-password` request:

```json
{
  "username": "demo"
}
```

`POST /forgot-password` success response:

```json
{
  "status": "ok",
  "message": "If the account exists, reset instructions have been issued",
  "reset_token": "<reset-token>"
}
```

`POST /reset-password` request:

```json
{
  "token": "<reset-token>",
  "new_password": "new-password"
}
```

`POST /reset-password` success response:

```json
{
  "status": "ok",
  "message": "Password reset"
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
- login success returns `200 OK` with a signed JWT and minimal subject identity
- login locks the account after three consecutive wrong password attempts
- change-password verifies the current password before updating it
- forgot-password always returns a generic success message and currently exposes the reset token directly for the PoC
- reset-password validates the reset token and updates the stored password
- me success returns `200 OK` with minimal subject identity derived from a validated active token
- logout success returns `200 OK` and revokes the current session in the local in-memory session store
- all authentication failures return `401 Unauthorized`
- reset/change password failures return a generic error body without exposing token/account detail
- login failure responses do not distinguish between unknown user, wrong password, locked account, or self-excluded account

## Demo Data

The current in-memory repository contains a single demo auth account:

Primary local login:

- username: `demo`
- password: `password`
- locked: `false`
- failed login attempts: `0`

Additional local/perf demo accounts are also seeded:

- usernames: `demo-2` through `demo-100`
- password: `password`
- locked: `false`

This is only suitable for local development. The repository is not shared across instances and is reset on restart.

## Current Session Model

Sessions are currently stored in [src/infra/in_memory_session_store.rs](C:\Dev\login-base\src\infra\in_memory_session_store.rs#L1). On successful login the service:

1. creates an in-memory session record
2. issues a JWT containing a `sid` claim
3. validates `GET /me` and `POST /logout` against both the JWT and the current session state

This is intentionally temporary. The in-memory session store is enough for local development, but revocation and active-session state will be lost on restart and are not shared across instances.

## Password Flows

`POST /change-password` is an authenticated credential-management flow that requires the current password.

`POST /forgot-password` and `POST /reset-password` are currently local-development flows:

- reset tokens are stored in memory on the auth account
- reset tokens are returned directly in the forgot-password response for PoC usability
- reset tokens expire after `15` minutes
- a successful reset clears failed-login state and unlocks the account

## Tests

The current automated coverage is focused on authentication behavior, password flows, session validation, and the HTTP boundary.

Application tests in [src/app/login/service.rs](C:\Dev\login-base\src\app\login\service.rs#L44) cover:

- JWT issuance after successful authentication
- propagation of authentication failures without token issuance
- core authentication success and failure branches
- unknown username
- wrong password
- lockout after repeated wrong passwords
- locked account
- self-excluded account
- change-password success and failure branches
- forgot-password reset-token issuance
- reset-password success and failure branches

Infrastructure tests cover:

- JWT claim issuance including `sid`
- active-session validation
- revoked-session rejection
- current-user lookup from an active session

HTTP tests in [src/http/routes.rs](C:\Dev\login-base\src\http\routes.rs#L54) cover:

- success response shape and `200` status
- generic failure response shape and `401` status
- the non-enumerating behavior for all failure branches
- change-password route behavior
- forgot-password route behavior
- reset-password route behavior
- me/logout session-backed behavior

Run the test suite with:

```bash
cargo test
```

## Perf Checks

Local perf checks are treated as regression indicators, not production benchmarks. The current setup uses a fixed `k6` scenario against the in-memory/mock application and stores a reduced summary for comparison over time.

Files:

- `perf/login.js`: fixed load scenario for `POST /login`
- `perf/login-failure.js`: fixed load scenario for wrong-password `POST /login`
- `perf/run.ps1`: runs `k6`, stores a summary in `perf/results`, and compares to `perf/baseline.json`
- `perf/promote-baseline.ps1`: promotes the latest or a chosen result into `perf/baseline.json`
- `perf/baseline.json`: baseline metrics and warn/fail guardrails
- `perf/baseline-login-failure.json`: optional baseline for the failed-login scenario

Default scenario:

- `20` virtual users
- `30s` duration
- successful login payload for the seeded demo auth account

Failed-login scenario:

- `20` virtual users
- `30s` duration
- wrong-password login payloads spread across `demo` through `demo-100`
- designed to stay on the bad-password path longer instead of immediately benchmarking a single locked account

Captured metrics:

- `p50`, `p95`, `p99`
- average latency
- throughput
- failure rate

Run it with the service already running locally:

```powershell
.\perf\run.ps1
```

Run the failed-login perf scenario:

```powershell
.\perf\run.ps1 -Scenario login-failure
```

Promote the latest result into the baseline:

```powershell
.\perf\promote-baseline.ps1
```

When the latest result is a failed-login run, promotion will target `perf/baseline-login-failure.json`.

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
- Session storage is currently in memory.
- Password reset tokens are currently in memory and returned in the forgot-password response for local use.
- CORS currently allows `http://localhost:5173`.
