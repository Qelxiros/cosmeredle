FROM rust:1.97.1

WORKDIR /app

COPY src src
COPY Cargo.toml .
COPY Cargo.lock .
COPY tags .

RUN cargo build --release

CMD ["target/release/cosmeredle"]
