# Fabelis MCP Rig Implementation

## Overview

The MCP (Modular Client Platform) project is designed to facilitate seamless communication between clients and servers using a modular architecture. It provides tools and services to enhance client-server interactions, including support for various protocols and integration with external APIs.

## Crates

- **Examples**: Examples of tool calling with `mcp-core` using a RIG agent.
- **MCP Rig**: A Fork of [RIG](https://github.com/0xPlaygrounds/rig) implementing **McpTool**.

## Environment
To run the MCP Agent, you need to set the `OPENAI_API_KEY` environment variable.
```bash
cp .env.example .env
```

## Usage Example
To post a message on Twitter using the MCP platform, simply prompt the agent with a command like "Post hello on twitter" and let the system handle the rest.