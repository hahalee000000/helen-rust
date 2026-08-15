# Multimodal Support (v1.17)

Helen v1.17 introduces callback-based multimodal support using the **callbacks-as-adapters** design pattern, enabling Helen to handle images, video, audio, and other media types while maintaining compatibility with various LLM providers.

## Design Principles

The core principle of multimodal support is: **protocol differences are handled by user callbacks; Helen core does not hardcode provider-specific formats**.

This design provides the following advantages:

- **Protocol-agnostic**: Helen core is not bound to any specific provider's media format
- **Extensible**: Adding new modalities or providers requires no changes to the language core
- **Flexible**: Users can customize media handling logic as needed
- **Backward compatible**: Pure text programs require no modifications

## Core Concepts

### MediaPart Data Type

`MediaPart` is a first-class citizen data type representing media content:

```helen
import std.media.*
main {
    // Create from file
    let img = media("file:///path/to/image.png")

    // Create from URL
    let remote_img = media("https://example.com/image.jpg")

    // Create from base64
    let b64_data = read_file_base64("image.png")
    let inline_img = media_base64(b64_data, "image/png")

    // Check if it is a MediaPart
    is_media(img)  // Returns: true

    // Get media type
    media_type(img)  // Returns: "image"
}
```

### MediaPart Fields

Each `MediaPart` object contains the following fields:

- `source`: Source type ("file", "url", "base64")
- `content`: Content (file path, URL, or base64 string)
- `mime`: MIME type (e.g., "image/png")
- `media_type`: Media type ("image", "video", "audio")
- `metadata`: Extra metadata (dictionary)

## llm act Multimodal Syntax

### Basic Usage

Passing media in `llm act`:

```helen
import std.core.*
import std.media.*
agent 图像分析 {
    description "Analyze image content"
    
    main {
        let img = media("photo.jpg")
        let result = llm act "Describe this image" media(img)
        print(result)
    }
}
```

### Multiple Media

You can pass multiple media objects:

```helen
import std.media.*
main {
    let img1 = media("image1.png")
    let img2 = media("image2.png")
    let result = llm act "Compare these two images" media(img1, img2)
}
```

### on_media Callback (Media Adapter)

The `on_media` callback converts a list of `MediaPart` objects into the format required by a specific provider. Helen provides three built-in format adapter stdlib functions — in most cases you never need to hand-write JSON:

```helen
import std.media.*
agent Claude媒体处理 {
    main {
        let img = media("diagram.png")
        
        // Recommended: use built-in format adapter (one line)
        let result = llm act "Explain this diagram" 
            media(img)
            on_media fn(parts, provider) { return 转Claude格式(parts) }
    }
}
```

**Built-in format adapters**:

| Function | Description |
|----------|-------------|
| `to_openai_parts(parts)` / `转OpenAI格式(parts)` | OpenAI-compatible format (default, usually no need to specify manually) |
| `to_claude_parts(parts)` / `转Claude格式(parts)` | Anthropic Claude Messages API format |
| `to_gemini_parts(parts)` / `转Gemini格式(parts)` | Google Gemini inline_data format |

**Custom adapters**: Only needed when using a non-standard provider or requiring special handling:

```helen
import std.list.*
main {
    on_media fn(parts, provider) {
        // Only hand-write for non-standard providers
        return parts.map(fn(part) {
            return {
                "type": "media",
                "mime_type": part.mime,
                "data": 媒体转base64(part),
                "encoding": "base64"
            }
        })
    }
}
```

**Parameters**:
- `parts`: List of `MediaPart` objects
- `provider`: Name of the current provider (e.g., "openai", "claude")
- **Return value**: Converted content parts list (provider-specific format)

**Default behavior**: If `on_media` is not specified, Helen uses the default OpenAI-compatible adapter (internally calls `to_openai_parts()`).

### on_generate Callback (Media Generation)

The `on_generate` callback registers media generation capability as a tool, letting the LLM decide when to invoke it:

```helen
import std.core.*
import std.media.*
agent 图像生成器 {
    description "Generate images from descriptions"
    
    main {
        let result = llm act "Create a sunset landscape image"
            on_generate fn(params) {
                // params contains: prompt, size, model, etc.
                let prompt = params["prompt"]
                
                // Call image generation API
                let image_url = call_image_generation_api(prompt)
                
                // Return the generated media
                return media("url://" + image_url)
            }
        
        print("Generated image: " + result)
    }
}
```

**How it works**:
1. `on_generate` registers the generation capability as an LLM-callable tool
2. The LLM decides whether to call the generation tool within the tool loop
3. When called, the callback function executes and returns the generated `MediaPart`
4. The generated media is automatically added to the conversation context

**Supported scenarios**:
- Text-to-image
- Text-to-video
- Any media type that can be generated via API

### provider Clause

Specifies the provider to use (affects default adapter behavior):

```helen
import std.media.*
main {
    let result = llm act "Analyze this image"
        media(img)
        provider("claude")
}
```

### Streaming Callbacks

Multimodal also supports streaming output callbacks:

```helen
import std.core.*
import std.media.*
main {
    let result = llm act "Describe this image in detail"
        media(img)
        on_chunk fn(chunk) {
            print(chunk, flush=false)
        }
        on_complete fn(full_text) {
            print("\nDone!")
        }
}
```

## Chinese Aliases

All multimodal-related functions support Chinese aliases:

| English | Chinese |
|---------|---------|
| `media()` | `媒体()` |
| `media_base64()` | `媒体base64()` |
| `is_media()` | `是媒体()` |
| `media_type()` | `媒体类型()` |
| `on_media fn(...)` | `处理媒体 fn(...)` |
| `on_generate fn(...)` | `生成 fn(...)` |
| `to_openai_parts()` | `转OpenAI格式()` |
| `to_claude_parts()` | `转Claude格式()` |
| `to_gemini_parts()` | `转Gemini格式()` |
| `media_to_base64()` | `媒体转base64()` |
| `save_media()` | `保存媒体()` |
| `is_image()` | `是图片()` |
| `is_video()` | `是视频()` |
| `is_audio()` | `是音频()` |

## Built-in Stdlib Function Reference

### Format Adapters

Convert a list of `MediaPart` objects into a specific provider's content format:

```helen
import std.media.*
main {
    // OpenAI-compatible format (default, usually no need to specify manually)
    let parts = to_openai_parts(media_list)

    // Anthropic Claude Messages API format
    let parts_claude = to_claude_parts(media_list)
    // Note: Claude does not support video and audio input; ValueError will be raised

    // Google Gemini inline_data format
    let parts_gemini = to_gemini_parts(media_list)
}
```

### Media Utilities

```helen
import std.media.*
main {
    // Convert any MediaPart to a pure base64 string (regardless of source: file/url/base64)
    let b64 = media_to_base64(img)

    // Save MediaPart to a file (path is optional; defaults to ~/.helen/generated_media/)
    let path = save_media(img, "/tmp/output.png")
    let path2 = save_media(img)  // Auto-named
}
```

### Type Predicates

```helen
import std.core.*
import std.media.*
main {
    if 是图片(part) { print("This is an image") }
    if 是视频(part) { print("This is a video") }
    if 是音频(part) { print("This is audio") }

    // Non-MediaPart safe: returns false, does not throw
    is_image("not media")  // Returns: false
}
```

## Complete Examples

### Image Analysis Agent

```helen
import std.core.*
import std.media.*
agent 图像分析助手 {
    description "A professional image analysis assistant that can understand and describe image content"
    model "qwen-vl-max"
    
    main {
        // Get image path from user
        let image_path = input("Please enter the image path: ")
        
        // Create MediaPart
        let img = media(image_path)
        
        // Analyze the image
        let analysis = llm act "Please describe the content, style, and possible uses of this image in detail"
            media(img)
        
        print("\nAnalysis result:\n" + analysis)
    }
}
```

### Multi-Image Comparison Agent

```helen
import std.core.*
import std.media.*
agent 图像比较器 {
    description "Compare differences between multiple images"
    
    main {
        let img1 = media("before.png")
        let img2 = media("after.png")
        
        let comparison = llm act "Compare these two images, pointing out the main changes and differences"
            media(img1, img2)
        
        print(comparison)
    }
}
```

### Image Generation Agent

```helen
import std.core.*
import std.media.*
agent 创意图像生成器 {
    description "Generate images from text descriptions"
    
    main {
        let description = input("Describe the image you want to generate: ")
        
        let result = llm act description
            on_generate fn(params) {
                // Here you should call the actual image generation API
                // This example uses a placeholder
                let prompt = params["prompt"]
                let api_response = call_dalle_api(prompt, size="1024x1024")
                
                // Return the generated image
                return media(api_response["url"])
            }
        
        print("Generated image: " + result)
    }
}
```

### Custom Provider Adaptation

Using built-in format adapters (recommended):

```helen
import std.core.*
import std.media.*
agent Claude分析 {
    main {
        let img = media("chart.png")
        
        // Use the built-in Claude adapter — one line
        let result = llm act "Analyze this chart"
            media(img)
            on_media fn(parts, provider) { return 转Claude格式(parts) }
        
        print(result)
    }
}
```

Hand-writing an adapter for a non-standard provider:

```helen
import std.list.*
import std.media.*
agent 自定义媒体处理 {
    main {
        let img = media("chart.png")
        
        let result = llm act "Analyze this chart"
            media(img)
            provider("custom_provider")
            on_media fn(parts, provider) {
                // Use 媒体转base64 helper, hand-write provider-specific format
                return parts.map(fn(part) {
                    return {
                        "type": "media",
                        "mime_type": part.mime,
                        "data": 媒体转base64(part),
                        "encoding": "base64"
                    }
                })
            }
    }
}
```

## Session Recording Integration

Multimodal content is fully integrated into Helen's session recording system:

- **Automatic persistence**: All multimodal conversations are automatically saved to `~/.helen/sessions/`
- **Large media external storage**: Base64 media >= 1MB is automatically extracted to external files
- **Session recovery**: Conversations containing media can be fully restored after restarting Helen
- **Compression safe**: Context compression correctly handles multimodal content

Configure the external storage threshold (`~/.helen/config.yaml`):

```yaml
multimodal:
  max_media_size_mb: 20              # Max 20MB per single media
  max_media_per_request: 10          # Max 10 media per request
  media_external_threshold_mb: 1.0   # >= 1MB extracted to external file
  media_cache_dir: "~/.helen/media_cache"
  video_frame_interval: 1.0          # Video frame extraction interval (seconds)
```

## Best Practices

### 1. Use Built-in Format Adapters

For mainstream providers, use the built-in adapters directly — no need to hand-write JSON:

```helen
import std.media.*
main {
    // OpenAI-compatible provider (default, no on_media needed)
    let result = llm act "Analyze the image" media(img)

    // Claude — one-line adapter
    let result_claude = llm act "Analyze the image"
        media(img)
        on_media fn(parts, provider) { return 转Claude格式(parts) }

    // Gemini
    let result_gemini = llm act "Analyze the image"
        media(img)
        on_media fn(parts, provider) { return 转Gemini格式(parts) }

    // Only hand-write on_media when a non-standard provider is needed
}
```

### 1.5 Leverage Media Utility Functions

`media_to_base64()` and `save_media()` are especially useful in `on_generate` callbacks:

```helen
import std.media.*
import std.network.*
main {
    on_generate fn(params) {
        let resp = http_post("https://api.example.com/generate", {...})
        let img = media_base64(resp.image_data, "image/png")
        
        // Save to specified path
        保存媒体(img, params["output_path"])
        
        // Or get base64 for further processing
        let b64 = 媒体转base64(img)
        
        return img
    }
}
```

### 1.6 Use Type Predicates for Filtering

When processing mixed media lists, type predicates enable precise filtering:

```helen
import std.list.*
main {
    let parts = [img1, video1, audio1, img2]
    let images = parts.filter(是图片)    // Keep only images
    let videos = parts.filter(是视频)    // Keep only videos
}
```

### 2. Manage Large Media Reasonably

Large media files are automatically stored externally, but you can control this manually:

```helen
import std.media.*
main {
    // Small image (<1MB): inline storage
    let small_img = media("icon.png")

    // Large image (>=1MB): automatic external storage
    let large_img = media("high_res_photo.png")
}
```

### 3. Error Handling

Handle potential media loading errors:

```helen
import std.core.*
import std.media.*
main {
    try {
        let img = media("possibly_nonexistent_file.png")
        let result = llm act "Analyze" media(img)
    } catch err {
        print("Media loading failed: " + err.message)
    }
}
```

### 4. Batch Processing

When processing multiple media, be aware of provider limits:

```helen
import std.media.*
main {
    // Default max 10 media per request
    let images = [media("img1.png"), media("img2.png"), media("img3.png")]
    let result = llm act "Analyze these images" media(images)
}
```

> **Dynamic lists**: `media()` accepts `list[MediaPart]` and auto-flattens, no spread syntax needed.
> `media(images)` is equivalent to `media(img1, img2, img3)`, suitable for scenarios where the image count is only determined at runtime.

## Limitations and Caveats

1. **Provider support**: Not all LLM providers support multimodal; you need to confirm provider capabilities
2. **File size**: Default max 20MB per media file, adjustable in configuration
3. **Network media**: URL media requires network access and may need timeout handling
4. **Format compatibility**: Different providers support different media formats; appropriate conversion is needed

## Related Resources

- [TranscriptStore User Guide](../docs/transcript_store_user_guide.md)
- [helen-syntax skill](../skills/software-development/helen-syntax/SKILL.md)
- [helen-stdlib skill](../skills/software-development/helen-stdlib/SKILL.md)
- [Multimodal Proposal](../reports/multimodal-proposal.md)
