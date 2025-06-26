FROM rust 
RUN USER=root apt-get update && \
    apt-get -y upgrade && \
    apt-get -y install git curl g++ build-essential libssl-dev pkg-config && \
    apt-get -y install software-properties-common && \
    apt-get update 

RUN USER=root cargo new --bin twilight-relayer-rust
RUN cd ./twilight-relayer-rust
WORKDIR /twilight-relayer-rust

COPY ./Cargo.toml ./Cargo.toml

# this build step will cache your dependencies
RUN cargo run
RUN rm src/*.rs

# copy your source tree
COPY ./src ./src
COPY ./.env ./.env


# RUN cargo build --release
# WORKDIR /twilight-relayer-rust
EXPOSE 3030
# COPY ./.env ./.env
# RUN cargo run
# ENTRYPOINT ["/usr/local/bin/aeronmd"]
CMD ["cargo", "run"]