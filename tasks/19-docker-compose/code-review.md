# Brooks-Lint Review — code (docker-compose vs main)

## Score 94/100

- 🟡 Domain Model Distortion (security posture): `ports: "${NTR_PORT}:${NTR_PORT}"` publishes
  to 0.0.0.0 on the host, so `docker compose up` exposes the explicitly-unauthenticated
  server to the LAN — contradicting the app's loopback-by-default (HOST=127.0.0.1) and the
  README's "keep the default loopback bind ... before exposing it".
  Remedy: publish to loopback by default, opt-in to expose:
  `"${NTR_BIND:-127.0.0.1}:${NTR_PORT:-8080}:${NTR_PORT:-8080}"` (set NTR_BIND=0.0.0.0 for
  multi-device). In-container HOST=0.0.0.0 stays (Docker needs it to forward).
- 🟢 Knowledge Duplication: ${NTR_PORT:-8080} default repeated 4x. Accepted single-file
  tradeoff (optional .env).

Fix plan: review-plan.md. Implementing the 🟡.
