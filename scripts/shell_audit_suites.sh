#!/usr/bin/env bash

append_core_cases() {
  local ls_dir node_dir make_dir rust_dir git_dir docker_compose_dir docker_image_dir docker_runtime_dir

  setup_ls_fixture
  ls_dir="$SETUP_CWD"
  setup_node_fixture
  node_dir="$SETUP_CWD"
  setup_make_fixture
  make_dir="$SETUP_CWD"
  setup_rust_fixture
  rust_dir="$SETUP_CWD"
  setup_git_fixture
  git_dir="$SETUP_CWD"

  append_case "generic.ls" "$ls_dir" ls -la
  append_case "generic.find" "$repo_root" find src/shell -maxdepth 2 -type f
  append_case "generic.grep" "$repo_root" grep summarize src/shell/patterns/git -R
  append_case "generic.env" "$repo_root" env
  append_case "git.status" "$git_dir" git status
  append_case "git.log" "$git_dir" git log -n 3
  append_case "git.remote" "$git_dir" git remote -v
  append_case "rust.cargo-test" "$rust_dir" cargo test
  append_case "rust.cargo-check" "$rust_dir" cargo check

  command -v npm >/dev/null 2>&1 && append_case "node.npm" "$node_dir" npm run demo || true
  command -v pnpm >/dev/null 2>&1 && append_case "node.pnpm" "$node_dir" pnpm run demo || true
  command -v bun >/dev/null 2>&1 && append_case "node.bun" "$node_dir" bun run demo || true
  command -v make >/dev/null 2>&1 && append_case "build.make" "$make_dir" make test || true

  if command -v docker >/dev/null 2>&1; then
    setup_docker_compose_fixture
    docker_compose_dir="$SETUP_CWD"
    append_case "docker.compose" "$docker_compose_dir" docker compose config --services
    if docker info >/dev/null 2>&1; then
      setup_docker_image_fixture
      docker_image_dir="$SETUP_CWD"
      append_case "docker.images" "$docker_image_dir" docker images --filter reference=so-context-shell-audit
      setup_docker_runtime_fixture
      docker_runtime_dir="$SETUP_CWD"
      append_case "docker.logs" "$docker_runtime_dir" docker logs "$docker_live_logger"
    fi
  fi

  if command -v gh >/dev/null 2>&1; then
    append_case "gh.pr" "$repo_root" gh pr list --repo cli/cli --limit 3
    append_case "gh.issue" "$repo_root" gh issue list --repo cli/cli --limit 3
    append_case "gh.run" "$repo_root" gh run list --repo cli/cli --limit 3
  fi

  if command -v kubectl >/dev/null 2>&1 && setup_k8s_fixture >/dev/null 2>&1; then
    append_case "k8s.kubectl-pods" "$repo_root" kubectl get pods -n "$k8s_namespace"
    append_case "k8s.kubectl-services" "$repo_root" kubectl get services -n "$k8s_namespace"
    append_case "k8s.kubectl-logs" "$repo_root" kubectl logs "$k8s_pod_name" -n "$k8s_namespace"
  fi
}

append_extended_cases() {
  local k8s_dir docker_runtime_dir

  append_setup_case "generic.rg" setup_generic_fixture rg TODO
  append_setup_case "generic.cat" setup_generic_fixture cat README.md
  append_setup_case "generic.head" setup_generic_fixture head -n 2 README.md
  append_setup_case "generic.tail" setup_generic_fixture tail -n 2 README.md
  append_setup_case "generic.curl.body" setup_generic_fixture curl -s "file://$repo_root/scripts/pattern_compare/fixtures/generic/curl-text.txt"
  append_setup_case "generic.curl.json" setup_generic_fixture curl -s "file://$repo_root/scripts/pattern_compare/fixtures/generic/curl-json.txt"
  if command -v wget >/dev/null 2>&1; then
    append_setup_case "generic.wget" setup_generic_fixture wget "data:text/plain,hello"
  else
    append_skipped_case "generic.wget" "wget data:text/plain,hello" "wget not installed"
  fi

  append_setup_case "git.diff" setup_git_diff_unstaged git diff
  append_setup_case "git.show" setup_git_log_history git show HEAD
  append_setup_case "git.branch" setup_git_branch_with_remote git branch -a
  append_setup_case "git.checkout" setup_git_branch_transition git checkout feat/shell
  append_setup_case "git.switch" setup_git_branch_transition git switch feat/shell
  append_setup_case "git.fetch" setup_git_fetch_new_ref git fetch --tags
  append_setup_case "git.pull" setup_git_pull_fast_forward git pull
  append_setup_case "git.push" setup_git_push_ahead git push origin main
  append_setup_case "git.add" setup_git_add_paths git add README.md notes.txt
  append_setup_case "git.commit" setup_git_commit_staged git commit -m "demo commit"
  append_setup_case "git.stash.save" setup_git_stash_save git stash -u
  append_setup_case "git.stash.list" setup_git_stash_list git stash list
  append_setup_case "git.clone" setup_git_clone_basic git clone ../remote.git cloned-repo
  append_setup_case "git.merge" setup_git_merge_conflict git merge feat/conflict
  append_setup_case "git.tag" setup_git_tag_list git tag
  append_setup_case "git.reset" setup_git_reset_unstage git reset HEAD README.md src/app.txt
  append_setup_case "git.status.dirty" setup_git_status_dirty git status

  append_setup_case "rust.cargo-build" setup_rust_fixture cargo build
  append_setup_case "rust.cargo-clippy" setup_rust_fixture cargo clippy --all-targets --no-deps
  append_setup_case "rust.cargo-install" setup_rust_fixture cargo install --path . --root ./install-root --force
  if cargo nextest --version >/dev/null 2>&1; then
    append_setup_case "rust.cargo-nextest" setup_rust_fixture cargo nextest run
  else
    append_skipped_case "rust.cargo-nextest" "cargo nextest run" "cargo nextest not installed"
  fi

  command -v yarn >/dev/null 2>&1 && append_setup_case "node.yarn" setup_node_fixture yarn run demo || append_skipped_case "node.yarn" "yarn run demo" "yarn not installed"
  append_setup_case "node.npx" setup_node_fixture npx --version

  if command -v docker >/dev/null 2>&1; then
    append_setup_case "docker.compose.config" setup_docker_compose_fixture docker compose config
    append_setup_case "docker.compose.images" setup_docker_compose_fixture docker compose config --images
    if docker info >/dev/null 2>&1; then
      append_setup_case "docker.build" setup_docker_image_fixture docker build .
      append_setup_case "docker.inspect" setup_docker_image_fixture docker inspect so-context-shell-audit:latest
      setup_docker_runtime_fixture >/dev/null 2>&1
      docker_runtime_dir="$SETUP_CWD"
      append_case "docker.ps" "$docker_runtime_dir" docker ps --filter "label=com.docker.compose.project=$docker_live_project"
    else
      append_skipped_case "docker.build" "docker build ." "docker daemon unavailable"
      append_skipped_case "docker.inspect" "docker inspect so-context-shell-audit:latest" "docker daemon unavailable"
      append_skipped_case "docker.ps" "docker ps --filter label=com.docker.compose.project=$docker_live_project" "docker daemon unavailable"
    fi
    append_skipped_case "docker.pull" "docker pull nginx:latest" "external registry dependency not part of formal audit"
  else
    append_skipped_case "docker.build" "docker build ." "docker not installed"
    append_skipped_case "docker.inspect" "docker inspect so-context-shell-audit:latest" "docker not installed"
    append_skipped_case "docker.ps" "docker ps --filter label=com.docker.compose.project=$docker_live_project" "docker not installed"
    append_skipped_case "docker.pull" "docker pull nginx:latest" "docker not installed"
  fi

  if command -v kubectl >/dev/null 2>&1 && setup_k8s_fixture >/dev/null 2>&1; then
    k8s_dir="$SETUP_ROOT"
    append_case "k8s.kubectl-describe" "$repo_root" kubectl describe pod "$k8s_pod_name" -n "$k8s_namespace"
    append_case "k8s.kubectl-apply" "$repo_root" kubectl apply -f "$k8s_dir/fixture.yaml" -n "$k8s_namespace"
  else
    append_skipped_case "k8s.kubectl-describe" "kubectl describe pod $k8s_pod_name -n $k8s_namespace" "k8s fixture unavailable"
    append_skipped_case "k8s.kubectl-apply" "kubectl apply -f fixture.yaml -n $k8s_namespace" "k8s fixture unavailable"
  fi

  append_skipped_case "build.tsc" "tsc --noEmit" "tooling-dependent audit case not available in this environment"
  append_skipped_case "build.next" "next build" "tooling-dependent audit case not available in this environment"
  append_skipped_case "build.vite" "vite build" "tooling-dependent audit case not available in this environment"
  append_skipped_case "build.gradle" "gradlew build" "tooling-dependent audit case not available in this environment"
  append_skipped_case "build.maven" "mvn test" "tooling-dependent audit case not available in this environment"
  append_skipped_case "build.dotnet-build" "dotnet build" "tooling-dependent audit case not available in this environment"
  append_skipped_case "build.dotnet-test" "dotnet test" "tooling-dependent audit case not available in this environment"
  append_skipped_case "build.dotnet-restore" "dotnet restore" "tooling-dependent audit case not available in this environment"
  append_skipped_case "build.dotnet-format" "dotnet format" "tooling-dependent audit case not available in this environment"
  append_skipped_case "build.cmake" "cmake --build build" "tooling-dependent audit case not available in this environment"
}
