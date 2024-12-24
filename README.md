# 🐱 Clara - AI Cat Artist Bot

An AI-powered Twitter bot that transforms user avatars into cute cat illustrations with stories.

## 💡 Features

- Transforms user avatars into cat-style illustrations
- Generates child-friendly stories
- Built with RIG framework
- Uses multiple AI services (Vision AI, DALL-E, GPT-4)

## 📌 Technical Stack

- Framework: [RIG](https://github.com/0xPlaygrounds/rig)
- APIs:
  - Twitter API
  - Google Vision AI
  - DALL-E
  - GPT-4

## ⭐️ Requirements

- Rust 1.70+
- Windows Server Environment
- 200GB RAM
- API Keys for all services

## ⚙️ Installation

```bash
git clone https://github.com/yourusername/clara-bot
cd clara-bot
cargo build --release
```

## 🛠 Configuration

Create a `.env` file in the project root:

```bash
TWITTER_API_KEY=your_key
VISION_API_KEY=your_key
DALLE_API_KEY=your_key
GPT4_API_KEY=your_key
```

## 📦 Usage

```bash
cargo run --release
```

## 📁 License

- MIT License

## 📬 Contributing

Pull requests are welcome. For major changes, please open an issue first.
