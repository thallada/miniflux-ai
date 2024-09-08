# Miniflux AI Summarizer

This Cloudflare Workers tool automatically adds AI-generated summaries to articles in your Miniflux RSS reader. The summaries are generated using the OpenAI API and appended to articles in a user-friendly format.

## Features

- **Automated Summarization**: Fetches unread articles from Miniflux, generates concise summaries using AI, and updates the articles with the summaries.
- **Customizable**: Configure the list of whitelisted websites, API endpoints, and AI model parameters through environment variables.
- **Concurrency**: Uses asynchronous Rust features to handle multiple articles concurrently, ensuring quick processing.
- **Cloudflare Integration**: Deployed as a serverless function on Cloudflare Workers, leveraging the scalability and performance of Cloudflare's global network.
- **Recommended Model**: Uses the Cloudflare Workers AI model `@cf/qwen/qwen1.5-14b-chat-awq` for generating high-quality, concise summaries.

## Getting Started

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) installed
- A Miniflux instance with API access
- An OpenAI account with access to the model endpoint
- A Cloudflare account

### Installation

1. Clone the repository:

   ```bash
   git clone https://github.com/zhu327/miniflux-ai.git
   cd miniflux-ai
   ```

2. Create the Cloudflare Worker KV namespace:

   ```bash
   npx wrangler kv:namespace create entries
   ```

3. Update the `wrangler.toml` with the KV namespace ID from the previous command (under the `kv_namespaces` section).

4. Deploy to Cloudflare Workers:

   ```bash
   npx wrangler deploy
   ```

5. Add the Miniflux Webhook integration. Copy the URL for your deployed cloudflare worker and paste it in the Miniflux Settings page under the Webhook integration. After saving the settings, there will be a text input containing the webhook secret key. Copy it and set it as the `MINIFLUX_WEBHOOK_SECRET` secret (see Configuration section below).

6. (Optional) To help differentiate the AI summary from the rest of the article text, add custom CSS styling for the `.ai-summary` block in your Miniflux settings (you may need to adjust the colors if you are not using a dark theme):

```css
.ai-summary {
  background-color: #222;
  border: 1px solid #aaa;
  padding: 8px;
  padding-top: 0px;
}
```

### Configuration

The tool is configured using environment variables and [worker secrets](https://developers.cloudflare.com/workers/configuration/secrets/).

See [this docs page on how to acquire your Cloudflare account ID and Workers API token](https://developers.cloudflare.com/workers-ai/get-started/rest-api/).

You can set worker secrets with the command:

```

npx wrangler secret put SECRET_NAME

```

#### Secrets

- `MINIFLUX_URL`: Your Miniflux instance URL.
- `MINIFLUX_USERNAME`: Your Miniflux username.
- `MINIFLUX_PASSWORD`: Your Miniflux password.
- `OPENAI_URL`: The endpoint for the OpenAI API.
- `OPENAI_TOKEN`: Your OpenAI API token.

#### Environment Variables

These environment variables can be set in the `wrangler.toml` file under the `[vars]` section:

- `OPENAI_MODEL`: The model ID to use for generating summaries. We recommend using the `@cf/qwen/qwen1.5-14b-chat-awq` model for best results.

### Usage

The worker listens for webhook requests from your Miniflux instance and when new entries are added it will store the entries in the Cloudflare KV store.

The worker also runs a scheduled cron job, querying the KV store for queued entries every 5 minutes. If the entry content is 500 characters or longer, it generates an AI summary and updates the article to include the summary at the top of the content.

### Contributing

Contributions are welcome! Please feel free to submit issues, feature requests, or pull requests.

### License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
