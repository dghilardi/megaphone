FROM denoland/deno:1.37.2

EXPOSE 3040
WORKDIR /app
USER deno

COPY demo/deno/chat-server.ts .
RUN deno cache chat-server.ts

CMD ["run", "--allow-net", "--allow-env", "chat-server.ts"]