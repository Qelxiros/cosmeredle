# Persistence lives outside this image. The container creates and migrates its
# own database under /app/storage at startup, and nothing here bakes one in, so
# replacing the container keeps whatever is on the mount.
#
# There is no way to attach the OKD volume from this file: VOLUME is a
# Docker/Podman-runtime instruction and CRI-O ignores it. The PVC has to be
# declared in the Deployment, mounted at /app/storage, because db::init opens
# storage/sqlite.db relative to the working directory.

FROM rust:1.97.1-bookworm AS builder

WORKDIR /app

# sqlx's query!/query_as! macros are type-checked against a real schema at
# compile time. `.sqlx/` carries that schema information with the source: it is
# recorded by `cargo sqlx prepare` against a migrated database and checked in,
# so the build needs no database of its own, no sqlite3 and no `cargo install
# sqlx-cli`. Rerun that command whenever a query or a migration changes, or the
# build stops with "no cached data for this query".
ENV SQLX_OFFLINE=true

# Dependencies are built against the manifest alone, so editing src/ does not
# invalidate the layer that compiles the whole tree of crates.
COPY Cargo.toml Cargo.lock build.rs ./
RUN mkdir src \
    && echo 'fn main() {}' > src/main.rs \
    && touch src/lib.rs \
    && cargo build --release \
    && rm -r src

COPY .sqlx .sqlx
COPY migrations migrations
COPY tags .
COPY src src

# COPY preserves source mtimes, which are older than the dependency build above,
# and cargo's fingerprints are mtime-based: without this it would consider the
# dummy crate fresh and ship that binary instead.
RUN find src -name '*.rs' -exec touch {} + \
    && cargo build --release

FROM debian:bookworm-slim AS runtime

# ca-certificates: the Coppermind sync talks TLS through rustls, which reads the
# system trust store. tzdata: the puzzle rolls over on Local::now(), which is
# UTC unless the zoneinfo database is present and TZ names a zone.
RUN apt-get update \
    && apt-get install --no-install-recommends --yes ca-certificates tzdata sqlite3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/cosmeredle /usr/local/bin/cosmeredle

# server::home reads src/index.html and the /static route serves src/static,
# both relative to the working directory, so the repository's layout is kept.
COPY src/index.html src/index.html
COPY src/static src/static

# Deliberately empty. With the PVC mounted here this is shadowed anyway; with
# nothing mounted the app still starts, and the data dies with the container.
#
# OKD's restricted-v2 SCC runs the container as an arbitrary UID whose group is
# 0, which is also what `USER 1001` resolves to under plain Docker, so group 0
# is what needs the write bit.
RUN mkdir -p /app/storage \
    && chgrp -R 0 /app \
    && chmod -R g=u /app

USER 1001
EXPOSE 3000

CMD ["/usr/local/bin/cosmeredle"]
