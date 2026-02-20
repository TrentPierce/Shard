import { NextResponse } from 'next/server'
import type { NextRequest } from 'next/server'

export function middleware(request: NextRequest) {
    const nonce = Buffer.from(crypto.randomUUID()).toString('base64')

    const cspHeader = `
    default-src 'self';
    script-src 'self' 'nonce-${nonce}' 'strict-dynamic' ${process.env.NODE_ENV === 'production' ? '' : "'unsafe-eval'"
        };
    style-src 'self' 'nonce-${nonce}' 'unsafe-inline' https://fonts.googleapis.com;
    font-src 'self' https://fonts.gstatic.com;
    connect-src 'self' http://127.0.0.1:* http://localhost:* ws://127.0.0.1:* ws://localhost:* *;
    img-src 'self' data: blob:;
    worker-src 'self' blob:;
    child-src 'self' blob:;
  `.replace(/\s{2,}/g, ' ').trim()

    const requestHeaders = new Headers(request.headers)
    requestHeaders.set('x-nonce', nonce)
    requestHeaders.set('Content-Security-Policy', cspHeader)

    const response = NextResponse.next({
        request: {
            headers: requestHeaders,
        },
    })

    response.headers.set('Content-Security-Policy', cspHeader)

    return response
}

export const config = {
    matcher: [
        {
            source: '/((?!api|_next/static|_next/image|favicon.ico).*)',
            missing: [
                { type: 'header', key: 'next-router-prefetch' },
                { type: 'header', key: 'purpose', value: 'prefetch' },
            ],
        },
    ],
}
