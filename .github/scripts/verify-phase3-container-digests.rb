#!/usr/bin/env ruby

require "yaml"

REPOSITORY_ROOT = ENV.fetch("GOSLING_CONTAINER_CHECK_ROOT", File.expand_path("../..", __dir__))
DIGEST_PATTERN = /@sha256:[0-9a-f]{64}\z/

EXPECTED_FROM = {
  ".devcontainer/Dockerfile" => [
    ["mcr.microsoft.com/devcontainers/rust:1@sha256:1707e2a8007968925f110c0961811200e9bb10e0ec055e2734857c59189a8b13", nil]
  ],
  "Dockerfile" => [
    ["rust:1.82-bookworm@sha256:d9c3c6f1264a547d84560e06ffd79ed7a799ce0bff0980b26cf10d29af888377", "builder"],
    ["debian:bookworm-slim@sha256:b1a741487078b369e78119849663d7f1a5341ef2768798f7b7406c4240f86aef", nil]
  ],
  "documentation/docs/docker/Dockerfile" => [
    ["rust:bullseye@sha256:9a11136145d74a2c7b2a74a36163fe9a58f392ef7eba15c2cb5b10e3ef13f361", "builder"],
    ["ubuntu:22.04@sha256:0e0a0fc6d18feda9db1590da249ac93e8d5abfea8f4c3c0c849ce512b5ef8982", nil]
  ],
  "services/ask-ai-bot/Dockerfile" => [
    ["oven/bun:1@sha256:e10577f0db68676a7024391c6e5cb4b879ebd17188ab750cf10024a6d700e5c4", "deps"],
    ["oven/bun:1@sha256:e10577f0db68676a7024391c6e5cb4b879ebd17188ab750cf10024a6d700e5c4", "build"],
    ["oven/bun:1@sha256:e10577f0db68676a7024391c6e5cb4b879ebd17188ab750cf10024a6d700e5c4", "production"]
  ],
  "ui/scripts/publish.sh" => [
    ["rust:1.92-bookworm@sha256:e90e846de4124376164ddfbaab4b0774c7bdeef5e738866295e5a90a34a307a2", nil]
  ]
}.freeze

TEST_FINDER_IMAGE = "ghcr.io/repo-makeover/gosling:sha-9f661a6@sha256:45c178cd40aceac2d3ea70bb99e0bcfaab584cdd758f7844dac9b0057f8e158c"
MANYLINUX_REFERENCES = [
  "quay.io/pypa/manylinux_2_28_x86_64@sha256:441c35fdc6ee809ff9260894f8468ab4fea8c15dc880f8700a3f81b7922c1cda",
  "quay.io/pypa/manylinux_2_28_aarch64@sha256:8b5f2b4e8c072ae5aefeb659f22c03e1ff46e6a82f154b6c904b106c87e65ff7"
].freeze

def fail_contract(message)
  warn "Phase 3 container check failed: #{message}"
  exit 1
end

def dockerfile_from(path)
  File.readlines(path, chomp: true).each_with_object([]) do |line, references|
    match = line.match(/^\s*FROM\s+(?:--platform=\S+\s+)?(\S+)(?:\s+AS\s+(\S+))?\s*$/i)
    next unless match

    references << [match[1], match[2]&.downcase]
  end
end

EXPECTED_FROM.each do |relative_path, expected_references|
  path = File.join(REPOSITORY_ROOT, relative_path)
  actual_references = dockerfile_from(path)

  unless actual_references == expected_references
    fail_contract("#{relative_path} FROM references are #{actual_references.inspect}; expected #{expected_references.inspect}")
  end

  actual_references.each do |reference, _stage|
    fail_contract("#{relative_path} contains an unpinned FROM reference: #{reference}") unless reference.match?(DIGEST_PATTERN)
  end
end

ask_ai_path = File.join(REPOSITORY_ROOT, "services/ask-ai-bot/Dockerfile")
ask_ai_lines = File.readlines(ask_ai_path, chomp: true)
ask_ai_from_indices = ask_ai_lines.each_index.select { |index| ask_ai_lines[index].match?(/^FROM\s+/) }

ask_ai_from_indices.each_with_index do |from_index, stage_index|
  next_index = ask_ai_from_indices.fetch(stage_index + 1, ask_ai_lines.length)
  stage_lines = ask_ai_lines[(from_index + 1)...next_index]
  fail_contract("Ask AI Bot stage #{stage_index + 1} does not declare WORKDIR /app") unless stage_lines.include?("WORKDIR /app")
end

test_finder_path = File.join(REPOSITORY_ROOT, ".github", "workflows", "test-finder.yml")
test_finder = YAML.load_file(test_finder_path)
actual_test_finder_image = test_finder.dig("jobs", "find-untested-code", "container", "image")

unless actual_test_finder_image == TEST_FINDER_IMAGE
  fail_contract("test-finder.yml image is #{actual_test_finder_image.inspect}; expected #{TEST_FINDER_IMAGE.inspect}")
end

build_cli = File.read(File.join(REPOSITORY_ROOT, ".github", "workflows", "build-cli.yml"))
MANYLINUX_REFERENCES.each do |reference|
  fail_contract("build-cli.yml lost pinned container #{reference}") unless build_cli.include?(reference)
end

dependabot_path = File.join(REPOSITORY_ROOT, ".github", "dependabot.yml")
dependabot = YAML.load_file(dependabot_path)
docker_updates = dependabot.fetch("updates").select { |update| update["package-ecosystem"] == "docker" }
expected_directories = ["/", "/.devcontainer", "/documentation/docs/docker", "/services/ask-ai-bot"]

unless docker_updates.length == 1 && docker_updates.first["directories"] == expected_directories
  fail_contract("Dependabot Docker directories are not the expected maintained set")
end

unless docker_updates.first.dig("schedule", "interval") == "weekly"
  fail_contract("Dependabot Docker digest updates must run weekly")
end

puts "Phase 3 container digest contracts passed."
