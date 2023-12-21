FROM rust:1.65.0 
# FROM rust
RUN USER=root apt-get update && \
    apt-get -y upgrade && \
    apt-get -y install git curl g++ build-essential libssl-dev pkg-config && \
    apt-get -y install software-properties-common && \
    apt-get update 

RUN  apt-get -y  install openssh-server
RUN  apt-get -y  install openssh-client    
COPY ./id_ed25519 root/.ssh/
COPY ./id_ed25519.pub root/.ssh/
RUN chmod 600 /root/.ssh/id_ed25519 && \
    chmod 600 /root/.ssh/id_ed25519.pub
RUN    touch /root/.ssh/known_hosts && \
    ssh-keyscan github.com >> /root/.ssh/known_hosts && \
    chmod 0600 /root/.ssh/id_ed25519 

RUN USER=root cargo new --bin rpckafka
RUN cd ./rpckafka
WORKDIR /rpckafka

# COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml
RUN rm src/*.rs
COPY ./src ./src
RUN cargo build --release
WORKDIR /rpckafka/target/release
COPY ./.env ./.env
EXPOSE 3032
ENTRYPOINT ["/rpckafka/target/release/rpckafka"]
