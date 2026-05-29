{
  description = "Taimen (対面) — open-source video conferencing server";

  inputs.substrate.url = "github:pleme-io/substrate";

  outputs = { substrate, ... }: substrate.rust.tool { src = ./.; };
}
