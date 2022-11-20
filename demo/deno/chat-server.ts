#! /usr/bin/env -S deno run --allow-net

import { Application, Router } from 'https://deno.land/x/oak@v11.1.0/mod.ts';

const megaphoneUrl = 'http://localhost:3000'

const port = 3040;
const app = new Application();

const rooms = new Map<string, string[]>();

const router = new Router();

router.post('/room/:room', async (ctx) => {
  const channelUuid = await fetch(`${megaphoneUrl}/create`, { method: 'POST' })
    .then((resp) => {
      if (!resp.ok) {
        throw new Error("HTTP status code: " + resp.status);
      }
      return resp.text();
    });

  const subscriptions = rooms.get(ctx.params.room);
  if (subscriptions) {
    rooms.set(ctx.params.room, [...subscriptions, channelUuid]);
  } else {
    rooms.set(ctx.params.room, [channelUuid]);
  }

  ctx.response.body = JSON.stringify({
    channelUuid,
  });
});

router.post('/send/:room', async (ctx) => {
  const subscriptions = rooms.get(ctx.params.room);
  if (!subscriptions) {
    ctx.response.body = JSON.stringify({ status: 'NOT_FOUND' });
    return;
  }
  const req = await ctx.request.body({ type: 'json' }).value;
  
  const promises = subscriptions.map(channelUuid => fetch(`${megaphoneUrl}/write/${channelUuid}/new-message`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ message: req.message }),
  }));

  await Promise.all(promises);
  ctx.response.body = JSON.stringify({ status: 'ok' });
});

app.use(router.allowedMethods());
app.use(router.routes());

app.addEventListener('listen', () => {
  console.log(`Listening on: localhost:${port}`);
});

await app.listen({ port });