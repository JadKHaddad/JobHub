FROM rust:1.75.0-bookworm as builder

WORKDIR /home/app

COPY src /home/app/src
COPY Cargo.toml /home/app/Cargo.toml
COPY Cargo.lock /home/app/Cargo.lock

RUN --mount=type=cache,target=/home/app/target \
    cargo test && cargo build --release && mv /home/app/target/release/job_hub /usr/local/bin/job_hub

FROM debian:bookworm as runner

RUN apt-get update && apt-get install -y libssl-dev \
    ca-certificates \
    git \
    python3=3.11.2-1+b1 \
    python3.11-venv=3.11.2-6 \
    python3-pip=23.0.1+dfsg-1

RUN addgroup --system app && adduser app --system --ingroup app

COPY ML_ETL /home/app/ML_ETL

RUN python3 -m venv /home/app/venv
RUN /home/app/venv/bin/pip install -r /home/app/ML_ETL/requirements.txt
RUN /home/app/venv/bin/pip install numpy --upgrade

COPY --from=builder /usr/local/bin/job_hub /usr/local/bin/job_hub

RUN chown app:app /home/app

USER app

WORKDIR /home/app

RUN echo '#!/bin/sh' > /home/app/entrypoint.sh && \
    echo '. /home/app/venv/bin/activate' >> /home/app/entrypoint.sh && \
    echo 'exec "$@"' >> /home/app/entrypoint.sh && \
    chmod +x /home/app/entrypoint.sh

ENTRYPOINT ["/home/app/entrypoint.sh", "job_hub"]

# DOCKER_BUILDKIT=1 docker build -t job_hub:latest . --progress=plain
# docker run --rm -it -p 3000:3000 job_hub:latest --api-token "token" --socket-address "0.0.0.0:3000" --projects-dir "/home/app/projects"