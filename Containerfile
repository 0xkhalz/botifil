FROM docker.io/library/rust:1.99-bookworm AS builder

WORKDIR /app

RUN rustup target add x86_64-unknown-linux-musl

ENV CARGO_BUILD_JOBS=2

COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src \
    && printf 'pub fn _stub() {}\n' > src/lib.rs \
    && printf 'fn main() {}\n' > src/main.rs \
    && cargo build --release --target x86_64-unknown-linux-musl --locked \
    && rm -rf src

COPY src ./src
RUN touch src/lib.rs src/main.rs \
    && cargo build --release --target x86_64-unknown-linux-musl --locked

FROM gcr.io/distroless/static-debian12:nonroot

COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/botifil /usr/local/bin/botifil

ENTRYPOINT ["/usr/local/bin/botifil"]
