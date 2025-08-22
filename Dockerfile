FROM rust as builder
RUN USER=root apt-get update && \
    apt-get -y upgrade && \
    apt-get -y install git curl g++ build-essential libssl-dev pkg-config && \
    apt-get -y install software-properties-common && \
    apt-get update

RUN git clone -b develop https://github.com/twilight-project/relayer-core.git
WORKDIR /relayer-core

RUN cargo build --release 

FROM rust
RUN apt-get update && apt-get install -y ca-certificates curl libpq-dev libssl-dev
EXPOSE 3031

WORKDIR /app
COPY --from=builder ./relayer-core/target/release/main ./
COPY --from=builder ./relayer-core/target/release/main.d ./
COPY ./.env .env
COPY ./relayerprogram.json ./relayerprogram.json
COPY ./wallet.txt ./wallet.txt
ENTRYPOINT ["/app/main"]