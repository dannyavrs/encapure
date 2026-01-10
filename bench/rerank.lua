-- wrk Lua script for load testing /rerank endpoint
wrk.method = "POST"
wrk.headers["Content-Type"] = "application/json"
wrk.body = [[
{
  "query": "What is machine learning?",
  "documents": [
    "Machine learning is a type of artificial intelligence that learns from data",
    "The weather is nice today",
    "Deep learning uses neural networks for complex pattern recognition",
    "Python is a popular programming language",
    "Neural networks are inspired by biological neurons",
    "The stock market fluctuates daily",
    "Supervised learning requires labeled training data",
    "Coffee is a popular morning beverage"
  ]
}
]]

-- Track response statistics
local responses = {}

function response(status, headers, body)
    responses[status] = (responses[status] or 0) + 1
end

function done(summary, latency, requests)
    io.write("\n----- Response Status Codes -----\n")
    for status, count in pairs(responses) do
        io.write(string.format("  %d: %d responses\n", status, count))
    end
end
