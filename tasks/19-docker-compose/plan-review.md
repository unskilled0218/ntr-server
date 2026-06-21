# Brooks-Lint Review — plan.md

## Round 1 — Score 99/100

- 🟢 Knowledge Duplication: the port default `:-8080` is repeated across 4 interpolations
  (PORT, both sides of ports, healthcheck). Variable centralizes the value; default repeated.
  Optional remedy: a committed .env. Kept inline as the single-file tradeoff. Acceptable.

Considered, not flagged: cargo-run-at-runtime (required by "no Dockerfile", scoped); curl in
healthcheck (documented buildpack-deps property); HOST=0.0.0.0 override (correct, commented);
target/registry volumes isolate container builds from host target/.

Above the >95 gate. Implementing.
