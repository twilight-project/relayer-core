FROM rust:1.61.0 
# FROM rust
RUN USER=root apt-get update && \
    apt-get -y upgrade && \
    apt-get -y install git curl g++ build-essential libssl-dev pkg-config && \
    apt-get -y install software-properties-common && \
    apt-get update 
RUN USER=root cargo new --bin querykafkapsql
RUN cd ./querykafkapsql
WORKDIR /querykafkapsql

# COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml
RUN rm src/*.rs
COPY ./src ./src
RUN cargo build --release
WORKDIR /querykafkapsql/target/release
COPY ./.env ./.env
EXPOSE 3032
ENTRYPOINT ["/querykafkapsql/target/release/querykafkapsql"]
