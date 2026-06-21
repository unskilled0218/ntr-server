# 19 - docker-compose — code-review fix plan

Addresses the 🟡 from code-review.md. The 🟢 (port default repetition) stays as the accepted
single-file tradeoff.

## Fix (🟡 security posture) — publish to host loopback by default

`docker compose up` currently publishes `0.0.0.0:8080`, exposing the unauthenticated server
to the LAN and contradicting the app's loopback-by-default posture (and the README security
note). Make exposure deliberate:

- `docker-compose.yml`, `ports`:
  `- "${NTR_BIND:-127.0.0.1}:${NTR_PORT:-8080}:${NTR_PORT:-8080}"`
  Default host bind is `127.0.0.1` (loopback only); `NTR_BIND=0.0.0.0` opts into LAN /
  multi-device access. Update the surrounding comment to explain `NTR_BIND`.
- The in-container `HOST=0.0.0.0` is unchanged — Docker needs the app on all interfaces
  inside the container to forward the published port; only the host-side bind interface
  changes. The healthcheck (in-container `localhost`) is unaffected.
- `README.md`: in the Docker Compose subsection, note that the server is published on
  `127.0.0.1` by default and show `NTR_BIND=0.0.0.0 docker compose up` for reaching it from
  other devices, with the existing no-auth security caveat applying when you do.

## Verification

- Re-run the structural lint: no tabs, even indentation, required keys present (now incl. the
  `NTR_BIND` default in the ports line).
- `cargo test` / `fmt` stay green (no app code).
- Reason through: default `up` -> host loopback only (matches app posture); `NTR_BIND=0.0.0.0
  up` -> LAN exposure (deliberate). Healthcheck still hits in-container `localhost`.

## Out of scope

- The .env extraction for the port default (accepted 🟢 tradeoff).
