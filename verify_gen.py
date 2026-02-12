
import requests
import json

url = "http://127.0.0.1:8000/v1/chat/completions"
data = {
    "messages": [{"role": "user", "content": "Hello, explain what Shard is in one sentence."}],
    "max_tokens": 20
}
response = requests.post(url, json=data)
print(json.dumps(response.json(), indent=2))
