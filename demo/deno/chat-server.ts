#! /usr/bin/env -S deno run --allow-net --allow-env

import { Application, Router } from 'https://deno.land/x/oak@v11.1.0/mod.ts';

const megaphoneUrl = Deno.env.get("MEGAPHONE_URL") || 'http://localhost:3000';
const port = 3040;
const app = new Application();

const rooms = new Map<string, string[]>();

const router = new Router();

router.post('/room/:room', async (ctx) => {
  const { channelId, agentName } = await fetch(`${megaphoneUrl}/create`, { method: 'POST' })
    .then((resp) => {
      if (!resp.ok) {
        throw new Error("HTTP status code: " + resp.status);
      }
      return resp.json();
    });

  const subscriptions = rooms.get(ctx.params.room);
  if (subscriptions) {
    rooms.set(ctx.params.room, [...subscriptions, channelId]);
  } else {
    rooms.set(ctx.params.room, [channelId]);
  }

  ctx.response.body = JSON.stringify({
    channelUuid: channelId,
    agentName
  });
});

router.post('/send/:room', async (ctx) => {
  const subscriptions = rooms.get(ctx.params.room);
  if (!subscriptions) {
    ctx.response.body = JSON.stringify({ status: 'NOT_FOUND' });
    return;
  }
  const req = await ctx.request.body({ type: 'json' }).value;
  const timestamp = new Date().toISOString();

  const unavailableChannels: number[] = [];
  const promises = subscriptions
    .map((channelUuid, idx) => fetch(`${megaphoneUrl}/write/${channelUuid}/new-message`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        sender: req.sender,
        timestamp,
        message: req.message,
      }),
    }).then(res => {
      if (res.status == 404) {
        console.warn(`subscription ${channelUuid} not found`);
        unavailableChannels.push(idx);
      }
    }))
    ;

  await Promise.all(promises);
  unavailableChannels
    .sort((a,b) => b - a)
    .forEach(idx => subscriptions.splice(idx, 1));

  ctx.response.body = JSON.stringify({ status: 'ok' });
});

app.use(router.allowedMethods());
app.use(router.routes());

app.addEventListener('listen', () => {
  console.log(`Listening on: localhost:${port}`);
});

await app.listen({ port });