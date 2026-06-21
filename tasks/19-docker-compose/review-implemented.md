# 19 - docker-compose — review fix implemented

Applied the 🟡 from code-review.md per review-plan.md.

## Fix — default to loopback publish (🟡 -> resolved)
- docker-compose.yml ports: "${NTR_BIND:-127.0.0.1}:${NTR_PORT:-8080}:${NTR_PORT:-8080}".
  Default host bind is loopback (matches the app's HOST=127.0.0.1 default and the README
  no-auth caveat); NTR_BIND=0.0.0.0 opts into LAN/multi-device. In-container HOST=0.0.0.0
  unchanged (Docker needs it to forward); healthcheck (in-container localhost) unaffected.
- README: documented NTR_BIND and the security caveat.

## Left as-is
- 🟢 ${NTR_PORT:-8080} default repeated (accepted single-file tradeoff).

## Verification
- Structural lint clean (no tabs, even indent, keys present incl. NTR_BIND); cargo fmt green.
- Docker unavailable here; real `docker compose up` left to the user.

Re-review: 99/100.
