FROM rust:slim-buster as build
WORKDIR /usr/src/subcs
COPY . .
RUN cargo build --release

FROM debian:buster-slim
COPY --from=build /usr/src/subcs/target/release/subcs /usr/bin/
CMD ["subcs"]

