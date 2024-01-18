FROM rust:1.75.0-bookworm as builder

WORKDIR /home/app

COPY src /home/app/src
COPY Cargo.toml /home/app/Cargo.toml
COPY Cargo.lock /home/app/Cargo.lock

RUN --mount=type=cache,target=/home/app/target \
    cargo test && cargo build --release && mv /home/app/target/release/job_hub /usr/local/bin/job_hub

FROM debian:bookworm as runner

RUN addgroup --system app && adduser app --system --ingroup app

COPY --from=builder /usr/local/bin/job_hub /usr/local/bin/job_hub

USER app

WORKDIR /home/app

ENTRYPOINT ["job_hub"]

# DOCKER_BUILDKIT=1 docker build -t job_hub:latest . --progress=plain
# docker run --rm -it job_hub:latest