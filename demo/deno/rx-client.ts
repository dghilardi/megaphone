#! /usr/bin/env -S deno run --allow-net

import { MegaphonePoller } from 'npm:megaphone-client@0.9.3';
import { firstValueFrom } from "npm:rxjs@7.6.0";

const poller = new MegaphonePoller('http://localhost:5173');
const o = await poller.newUnboundedStream<{ message: string, sender: string }>(async channel => {
    let res = await fetch("http://localhost:5173/room/test", {
        method: "POST",
        headers: {
            ...(channel ? { 'use-channel': channel } : {}),
        },
    }).then(res => res.json());

    return {
        channelId: res.channelUuid,
        streamIds: ['new-message'],
    }
});

const chunk = await firstValueFrom(o);
console.log(chunk.body.message);