FROM rust:1.97.1

WORKDIR /app

COPY src src
COPY migrations migrations
COPY Cargo.toml .
COPY Cargo.lock .
COPY tags .

ENV DATABASE_URL sqlite://storage/sqlite.db
RUN mkdir storage && touch storage/sqlite.db
RUN cargo install sqlx-cli
RUN sqlx migrate run
RUN cargo build --release

CMD ["target/release/cosmeredle"]
