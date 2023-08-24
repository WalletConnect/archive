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
# Optional
cp .env.example .env
nano .env
```

### Run all dependency-less tests

```bash
just lint test
```

### Storage integration tests (and tests above)

This requires starting Docker dependencies.

```bash
just lint run-storage-docker test-storage
```

### Relay integration tests (and tests above)

You must run the relay locally or pass an `ENVIRONMENT` value to use a different, already deployed, history server.

```bash
just lint run-storage-docker test-storage-relay
```

### Stop Docker dependencies

```bash
just stop-storage-docker
```

## Terraform

```bash
cp .env.terraform.example .env.terraform
nano .env.terrafom
```

```bash
source .env.terraform
nano terraform/terraform.tf # comment out `backend "remote"` block
git submodule update --init --recursive
terraform -chdir=terraform init
terraform -chdir=terraform workspace new dev
terraform -chdir=terraform workspace select dev
terraform -chdir=terraform apply -var-file="vars/$(terraform -chdir=terraform workspace show).tfvars"
```
