# Database migrations

This crate uses Toasty 0.11 with PostgreSQL. Run the following commands from the
repository root, where `Toasty.toml` configures the migration paths.

## Pulumi deployments

`pulumi up` builds a migration image and executes the `database-migrate` Cloud Run
Job before creating or updating the API. The job uses the database secret and
Direct VPC egress to reach the private PostgreSQL endpoint. Pulumi waits for the
job to succeed; migration failure blocks the API deployment.

The job executes on first deployment and when its image digest or database secret
version changes. An unchanged deployment does not execute it again. The CLI applies
committed migration files and skips previously applied migrations. Generate, review,
and commit schema changes before deploying; deployment does not generate migrations.

To execute the existing job manually from a machine with `gcloud` credentials:

```sh
gcloud run jobs execute "$(pulumi stack output databaseMigrationJobName)" \
  --region=asia-southeast1 --wait
```

Use the stack's configured region. Keep migrations compatible with the API currently
serving traffic; a later deployment failure does not undo applied migrations.

## Apply a schema change

1. Edit the relevant model in `crates/database/src/models/`. For example, add an
   optional description to `Cake`:

   ```rust
   pub description: Option<String>,
   ```

2. Set `DATABASE_URL` to the database you want to update, either in the root
   `.env` file or your shell. PostgreSQL must be running and the database must
   already exist:

   ```sh
   export DATABASE_URL='postgresql://postgres:postgres@localhost:5432/serverless_demo'
   ```

3. Generate a migration with a descriptive name:

   ```sh
   cargo run -p database --bin cli -- migration generate --name add_cake_description
   ```

   Toasty compares your registered models against the last saved snapshot. It
   creates a SQL file, a new snapshot, and an updated history entry under
   `crates/database/toasty/`. Generation does not apply the SQL to the database.

4. Review the generated SQL. With the existing `0000_basic_schema.sql` migration,
   the next migration in this example is:

   ```text
   crates/database/toasty/migrations/0001_add_cake_description.sql
   ```

   Check that it contains the intended changes. If you renamed a field or table,
   select the rename option when Toasty prompts so it preserves the existing data.

5. Apply pending migrations:

   ```sh
   cargo run -p database --bin cli -- migration apply
   ```

   Toasty records applied migrations in `__toasty_migrations` and skips them on
   subsequent runs.

6. Commit the model changes together with all generated artifacts:

   ```text
   crates/database/toasty/history.toml
   crates/database/toasty/migrations/<number>_<name>.sql
   crates/database/toasty/snapshots/<number>_snapshot.toml
   ```

   Apply those same committed migration files to other environments by setting
   their `DATABASE_URL` and running `migration apply`. Generate each schema change
   once; do not regenerate it separately for each environment.

## Existing data

Create a new migration for each change and leave previously applied migrations
unchanged. For a required column on a populated table, first add it as nullable,
backfill existing rows, then make it required in a later migration.

Use migrations to update an existing database. `push_schema` creates the complete
schema without tracking changes, while `migration reset` drops tables.

The `_MIGRATIONS` static in `src/bin/cli.rs` embeds SQL into the binary but does
not apply it automatically. The current CLI uses `ToastyCli` to read migration
files from disk and apply them when you run `migration apply`.

## Inspect the schema

```sh
cargo run -p database --bin cli -- migration snapshot
```

This prints the schema derived from the current models; it does not inspect the
live database or apply changes. The current CLI opens a PostgreSQL connection
before dispatching commands, including generation and snapshot inspection.

See the [Toasty schema management guide](https://tokio-rs.github.io/toasty/0.11.0/guide/schema-management.html)
for the complete workflow.

## Scheduled mock data and cleanup

Every model includes a required UTC `created_at`. Migration
`0002_add_all_created_at.sql` backfills existing fruit, filling, and cake-filling
rows with the migration time before making their new columns required.

`database::scheduled` creates mock data in a transaction, selects keys older than
eight hours, and implements idempotent batch deletion for the Pub/Sub receiver.
Cleanup uses explicit, parameterized SQL to avoid ORM cascades removing younger
related rows. Newer required links keep their parent until a later cleanup;
optional fruit links can be detached. Older links arriving in another Pub/Sub
message cause parent deletion to retry.

Run unit tests with `cargo test --workspace`. To run the PostgreSQL integration
test, first apply migrations to an **empty, disposable** database using the CLI,
then set `CLEANUP_TEST_DATABASE_URL` to that database and run:

```sh
cargo test -p database --test scheduled -- --ignored
```

This test populates the database and checks strict age boundaries, all four
models, repeated messages, unordered delivery, and protection of newer related
records. It requires empty tables and leaves its test data in the database.
