FROM gitpod/workspace-full:latest

USER root
RUN apt-get install -yq \
        musl \
    && apt-get clean && rm -rf /var/lib/apt/lists/* /tmp/*

USER gitpod
RUN cd /home/gitpod && \
    .cargo/bin/rustup target add x86_64-unknown-linux-musl

USER root