# OpenPID
## OpenPID: Peripheral Interface Documentation
Our goal is to standardize how we document the communication interfaces of embedded peripherals.

## Quick-Start

## Examples

## Known Users

## License: GPL


## Credits:
OpenAPI is used as a source of inspiration.
Alyron's async-stripe uses Stripe's openAPI specification and emits Rust, creating an ergonomic and nearly feature-complete SDK with

The same OpenAPI spec is used to generate documentation. Anyone can consume an OpenAPI spec and use it to generate client code in any language. 

PlatformIO has different scope, and is focused on C/C++ embedded devices. It doesn't enforce any particular compatbility for sensor libraries though, so finding sensor libararies compatible with your platform/RTOS can be difficult

## Sales Pitch & Pipe Dream
Clean, generated-code can take advantage of a programming lanugage's recent features (like async), be idiomatic to at least some baseline, and guarantee the software developer some level of predictability.
The same config, makes documentation consistent with code accross languages and platforms. If a bug is found in one platform, it will exist in the documentation, and accross all other platforms, meaning just one config file change will fix it all.

Of course, this hinges on having correct codegen. We think that this is quite achievable, since time saved from not having to hand-write drivers can be spent improving codegen. We can take great care to make codegen really good, since it's an investment that will pay us back over and over again.

If you're a hobbyist, you populating an OpenPID file based on the documentation will quickly generate the SDK you need for your platform, and any other platform you have in your kit. 

Adding support for your platform will give you immediate access to all sensors that have spec files. Adding support for your sensor will give everyone who uses a current or future supported platform easy access to your sensor.

If you're a manufacturer, you can now reduce your maintanance and documentation overheads for SDKs and get your engineers back to focusing on the underlying product, or hustled along to R&D tasks. Meanwhile, get access to a larger section of the market, knowing that your libraries will support common and even esoteric platforms, from laptops, to GUI visualizations, embedded devices running weird RToSs, and rapidly growing embedded programming languages (like Rust!). Your sensors will be selected regardless of whether your driver software is currently compatible with a client's platform, but on the merits of the sensor itself.
