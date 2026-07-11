import { Innertube } from 'youtubei.js';

let innertubeInstance: Innertube | null = null;
let innertubePromise: Promise<Innertube> | null = null;

export async function getInnertube(): Promise<Innertube> {
  if (innertubeInstance) return innertubeInstance;
  if (!innertubePromise) {
    innertubePromise = Innertube.create({ generate_session_locally: true });
  }
  innertubeInstance = await innertubePromise;
  return innertubeInstance;
}
