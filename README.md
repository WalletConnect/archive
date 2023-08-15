# Gilgamesh

HTTP service managing users' e2e encrypted message histories.

This project also includes the standard CI/CD:
- Release
- Rust CI
- Terraform CI
- CD
- Intake
- Mocha (NodeJS) based integration tests

## Running the app

* Build: `cargo build`
* Test: `cargo test`
* Run: `docker-compose-up`
* Integration test: `yarn install` (once) and then `yarn integration:local(dev/staging/prod)`

## Testing

```bash
cp .env.example .env
nano .env
```

```bash
just lint run-storage-docker test-all stop-storage-docker
```

- [ ] TODO Consider making `lint` and `run-storage-docker` dependencies of `test`
