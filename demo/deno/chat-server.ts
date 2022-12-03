#! /usr/bin/env -S deno run --allow-net --allow-env

import { Application, Router } from 'https://deno.land/x/oak@v11.1.0/mod.ts';

interface ChannelParams {
  channelId: string,
  agentName: string,
}

const megaphoneUrl = Deno.env.get("MEGAPHONE_URL") || 'http://localhost:3000';
const megaphoneAgentUrlTemplate = Deno.env.get("MEGAPHONE_AGENT_URL") || 'http://localhost:3000';
const port = 3040;
const app = new Application();

const rooms: Record<string, ChannelParams[]> = {};

const router = new Router();

router.post('/room/:room', async (ctx) => {
  const { channelId, agentName }: ChannelParams = await fetch(`${megaphoneUrl}/create`, { method: 'POST' })
    .then((resp) => {
      if (!resp.ok) {
        throw new Error("HTTP status code: " + resp.status);
      }
      return resp.json();
    });

  const subscriptions = rooms[ctx.params.room];
  if (subscriptions) {
    rooms[ctx.params.room] = [...subscriptions, { channelId, agentName }];
  } else {
    rooms[ctx.params.room] = [{ channelId, agentName }];
  }

  ctx.response.body = JSON.stringify({
    channelUuid: channelId,
    agentName
  });
});

router.post('/send/:room', async (ctx) => {
  const subscriptions = rooms[ctx.params.room];
  if (!subscriptions) {
    ctx.response.body = JSON.stringify({ status: 'NOT_FOUND' });
    return;
  }
  const req = await ctx.request.body({ type: 'json' }).value;
  const timestamp = new Date().toISOString();

  const unavailableChannels: number[] = [];
  const promises = subscriptions
    .map(({ channelId, agentName }, idx) => fetch(`${megaphoneAgentUrlTemplate.replace('%agentName%', agentName)}/write/${channelId}/new-message`, {
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