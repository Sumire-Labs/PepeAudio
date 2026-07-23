// SPDX-License-Identifier: Apache-2.0
import { NextResponse, type NextRequest } from "next/server";

// One app, three faces by subdomain:
//   audio.*        -> /landing  (public marketing)
//   admin.*        -> /admin    (operator dashboard)
//   player.* / dev -> /         (the player app)
export function middleware(req: NextRequest) {
  const { pathname } = req.nextUrl;
  if (pathname.startsWith("/api") || pathname.startsWith("/hubs")
    || pathname.startsWith("/admin") || pathname.startsWith("/landing")) {
    return NextResponse.next();
  }

  const sub = (req.headers.get("host") ?? "").split(".")[0].toLowerCase();
  const target = sub === "admin" ? "/admin" : sub === "audio" ? "/landing" : null;
  if (target === null) return NextResponse.next();

  const url = req.nextUrl.clone();
  url.pathname = `${target}${pathname === "/" ? "" : pathname}`;
  return NextResponse.rewrite(url);
}

export const config = {
  matcher: ["/((?!_next/static|_next/image|favicon.ico).*)"],
};
