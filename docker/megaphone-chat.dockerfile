FROM denoland/deno:1.28.1

EXPOSE 3040
WORKDIR /app
USER deno

COPY demo/deno/chat-server.ts .
RUN deno cache chat-server.ts

CMD ["run", "--allow-net", "chat-server.ts"]