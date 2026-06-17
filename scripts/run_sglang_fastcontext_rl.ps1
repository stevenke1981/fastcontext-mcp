python -m sglang.launch_server `
  --model-path "microsoft/FastContext-1.0-4B-RL" `
  --tool-call-parser qwen `
  --context-length 262144 `
  --trust-remote-code `
  --dtype bfloat16 `
  --host 0.0.0.0 `
  --port 30000 `
  --tp-size 1 `
  --mem-fraction-static 0.8
