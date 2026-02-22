import { NextRequest } from "next/server"
import { POST as chatCompletionPost, GET as chatCompletionGet, OPTIONS as chatCompletionOptions } from "@/app/api/v1/chat/completions/route"

export const dynamic = "force-dynamic"

export async function GET() {
  return chatCompletionGet()
}

export async function POST(request: NextRequest) {
  return chatCompletionPost(request)
}

export async function OPTIONS() {
  return chatCompletionOptions()
}
