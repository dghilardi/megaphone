# Megaphone 📣
[![Test Status](https://github.com/dghilardi/megaphone/workflows/Tests/badge.svg?event=push)](https://github.com/dghilardi/megaphone/actions)
[![Crate](https://img.shields.io/crates/v/megaphone-broker.svg)](https://crates.io/crates/megaphone-broker)
[![API](https://docs.rs/megaphone-broker/badge.svg)](https://docs.rs/megaphone-broker)

Megaphone is a reverse proxy that allows clients to connect to a server using long running requests and server streaming.
It is useful because it abstracts the complexity of handling long running requests and server streaming from the server, allowing it to focus on the business logic.
It also gives the client a single endpoint to connect to, making it easier to manage the connection and reducing the opened connections and the overall traffic overhead.

## How it works

### Create a channel
To create a channel, the `[POST] /create` endpoint must be called.
Each channel:
 - has a buffer of 100 messages that will be used to keep messages pending if the consumer is not currently connected. If the buffer is full the server will keep eventual write operations pending for 10 seconds, after that it will respond with a `503 Service Unavailable` status code.
 - remains alive for 1 minute after the last read operation (or create if no read was performed). After that it is automatically deleted and all buffered messages are lost.
 - has two addresses, namely `producerAddress` and `consumerAddress`, the first one can be used to write into the channel, the second one to read from it.

### Write into a channel
To write into a channel, the client must call the `[POST] /write/{producer-address}/{stream-id}` endpoint.
The server will respond with a `201 Created` status code if the message was successfully written into the channel.

### Read from a channel
The only information needed to read from a channel is the `consumerAddress` returned when the channel was created.
At the moment the only supported protocol is http streaming, so to read from a channel the client must call the `[GET] /read/{consumer-address}` endpoint.

## Supported protocols
### Http Streaming
To access a channel using http streaming, the client must call the `[GET] /read/{consumer-address}` endpoint.
The server will keep the connection open for 20 seconds and send messages as they arrive.

### Other repos
- [Megaphone Client](https://github.com/dghilardi/megaphone-client) rust client that can be used to subscribe to megaphone channels.
- [Megaphone Client JS](https://github.com/dghilardi/megaphone-js) Javascript/Typescript client that can be used to subscribe to megaphone channels.
- [Megaphone Operator](https://github.com/dghilardi/megaphone-operator) Megaphone kubernetes operator
- [Megaphone Demo](https://github.com/dghilardi/megaphone-demo) Demo application that uses megaphone channels to implement a chat.