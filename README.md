# orgu
**orgu** is a tool for implementing organization-wide workflows on GitHub. It is pronounced *[ˈoɾɡu]*. [Blog post (ja)](https://techblog.finatext.com/orgu-e3a3ad0219a8).

## Why Choose orgu?

- Bridges the gap between GitHub's Team and Enterprise plans
- Enhances cost efficiency

GitHub and GitHub Actions are powerful tools. However, there's a significant disparity between GitHub Team and Enterprise plans. Users often seek organization-wide workflows, which are implemented as [GitHub's ruleset workflows](https://docs.github.com/en/enterprise-cloud@latest/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/available-rules-for-rulesets#require-workflows-to-pass-before-merging), but this feature requires an Enterprise plan. orgu addresses this by enabling users to host their own organization-wide workflows.

orgu offers a more cost-effective solution compared to GitHub Actions, particularly in terms of unit cost and billing period. GitHub Actions bill on a "per minute" basis, and since organization-wide workflows tend to run frequently, cost efficiency is crucial.

Organization-wide workflows in GitHub are CI jobs that operate across all or selected repositories within an organization, based on repository attributes. These workflows are triggered by events such as pull requests or check suite events. Although reusable workflows can be employed, they still require the distribution of workflow files to each repository individually. In contrast, organization-wide workflows eliminate the need for such distribution, allowing specific CI jobs to be executed at the organization level without the need to configure each repository separately. This enables a more streamlined and efficient management of continuous integration tasks across multiple projects.

## Architecture
![Architecture diagram](docs/images/architecture.png)

![orgu-runner sequence diagram](docs/images/runner-sequence.png)

orgu consists of two components:

- orgu-front
- orgu-runner

orgu-front captures events related to Check Suites or Pull Requests via webhooks linked to a specific GitHub App. Upon receiving the webhook event, orgu-front filters the event, constructs a `CheckRequest` object, and forwards it to an event queue for further action.

orgu-runner operates an event loop that retrieves events from this queue, triggering specified jobs based on each event. It reports the start, success, or failure of jobs via the GitHub Checks API. Additionally, orgu-runner manages repository checkouts and generates temporary GitHub tokens, enabling jobs to interact with the GitHub API or checkout other repositories as needed. Feedback on job outcomes is provided by the jobs themselves, allowing the use of specialized tools like [reviewdog](https://github.com/reviewdog/reviewdog) for reporting job results or [Opsgenie](https://www.atlassian.com/software/opsgenie) for alerts.

To execute organization-wide jobs, a unique container image must be created for each runner. This image should contain the orgu binary, all necessary job dependencies, and the job's execution script. Launching the orgu binary in "runner" mode within this container starts the orgu-runner's event loop.

See `example/Dockerfile` for more details on the orgu-runner container image.

### Default Platform
By default, orgu uses AWS Lambda functions and Amazon EventBridge Event Bus for event queuing.

### Filtering Events
In platforms like AWS Lambda, where billing occurs per invocation, users may want to minimize orgu-runner activations. By default, orgu filters `CheckRequest` elements generated from webhook events using [event patterns](https://docs.aws.amazon.com/eventbridge/latest/userguide/eb-event-patterns.html) in the EventBridge Event Bus. To help develop these event bus patterns, orgu includes commands like `orgu pattern generate` and `orgu pattern test`.

### Custom Properties Support
GitHub repositories feature [Custom Properties](https://docs.github.com/en/organizations/managing-organization-settings/managing-custom-properties-for-repositories-in-your-organization), allowing for the assignment of attributes. orgu supports these Custom Properties for filtering and referencing within jobs.

For filtering, orgu incorporates custom properties into `CheckRequest.repository.custom_properties`.

In runner jobs, orgu conveys custom properties as environment variables. For instance, the custom property `team=t-platform` translates to the `CUSTOM_PROP_TEAM=t-ferris` environment variable.

## Installation
Homebrew:

```
brew tap Finatext/orgu https://github.com/Finatext/orgu.git
brew install orgu
```

Download from GitHub release:

https://github.com/Finatext/orgu/releases

Cargo install:

```
cargo install --git https://github.com/Finatext/orgu.git
```

Build locally:

```
cargo install --path .
```

## Getting started
orgu can be run locally for development or testing. First, install the orgu binary by following the instructions in the "Installation" section.

Note: A GitHub organization is required to complete this tutorial.

### Forwarding Webhooks to Your Local Machine
To forward GitHub webhooks to a local machine, use a service like https://smee.io/. Generate a Webhook URL on the service, then execute the forwarding command below:

```
# orgu-front uses `/github/events` path
$ smee -u https://smee.io/<ID> -P /github/events
```

### Creating a GitHub App
Follow these steps to create a GitHub App:

1. **Create the App**: Follow the instructions in the GitHub documentation to create a GitHub App. Ensure it is set up to use webhooks. The "Webhook URL" should be the URL you created earlier.
2. **Set Permissions**: Refer to the "GitHub App Config" section in this documentation for the necessary permissions.
3. **Generate a Webhook Secret**: Use the following command to generate a random string for the "Webhook secret":

    ```sh
    $ ruby -r securerandom -e 'puts SecureRandom.hex(32)'
    ```

4. **Install the App**: Once the GitHub App is created, install it within your organization. For testing purposes, it is recommended to limit the repositories the app can access.

Refer to the following GitHub documentation for more details:

- [Registering a GitHub App](https://docs.github.com/en/apps/creating-github-apps/registering-a-github-app/registering-a-github-app)
- [Choosing Permissions for a GitHub App](https://docs.github.com/en/apps/creating-github-apps/registering-a-github-app/choosing-permissions-for-a-github-app)
- [Using Webhooks with GitHub Apps](https://docs.github.com/en/apps/creating-github-apps/registering-a-github-app/using-webhooks-with-github-apps)
- [Installing Your Own GitHub App](https://docs.github.com/en/apps/using-github-apps/installing-your-own-github-app)

### Creating a Local Environment File
To configure your environment, you can use any tool that allows setting environment variables. A dotenv-like tool is recommended for this purpose. Follow these steps:

1. **Install dotenv Tool**: Install a dotenv-like tool, such as `denv` or `dotenvx`:

    ```sh
    # Using denv
    $ gem install denv

    # Using dotenvx
    $ brew install dotenvx/brew/dotenvx
    ```

2. **Download the Private Key**: Download the private key from the settings page of the GitHub App you created earlier. Note that the private key consists of multiple lines. Remove line breaks for the next steps.

3. **Create a .env File**: Create a `.env` file and add the following environment variables. Replace the placeholders with actual values:

    ```sh
    $ cat <<EOS > .env
    GITHUB_APP_ID=<copy from GitHub App page>
    GITHUB_INSTALLATION_ID=<copy from GitHub App installation page>
    GITHUB_WEBHOOK_SECRET=<webhook secret value>
    GITHUB_PRIVATE_KEY=-----BEGIN RSA PRIVATE KEY-----your single line private key here-----END RSA PRIVATE KEY-----
    EOS
    ```

4. **Check Environment Variables**: Verify the environment variables using the following command:

    ```sh
    $ denv -- env
    ```

For more information on managing private keys for GitHub Apps, refer to the GitHub documentation:

- [Managing Private Keys for GitHub Apps](https://docs.github.com/en/apps/creating-github-apps/authenticating-with-a-github-app/managing-private-keys-for-github-apps)


### Run orgu-front
Execute `orgu front server` command with configured environment variables:

```
$ denv -- orgu front server
```

### Run orgu-runner
Execute `orgu runner server` command with configured environment variables:

```
$ denv -- orgu runner server --job-name=test -- ls
```

### Test orgu with PR
Create a test pull request in the configured repository. GitHub will send a webhook event to a forwarding service such as Smee, which then forwards the request to your local orgu-front server. In local development mode, the orgu-front server sends the `CheckRequest` payload directly to the local orgu-runner using an HTTP POST request. The orgu-runner executes the `ls` command in the checked-out repository, and the output of the `ls` command can be viewed in the orgu-runner's log entries.

Also the "Checks" tab in the pull request will have job execution logs.

If something went wrong:

- Check webhook forwarding service server and client logs
- Check webhook status in the "Recent Deliveries" page of GitHub App
  - `https://github.com/organizations/<org>/settings/apps/<app name>/advanced`
- Check the GitHUb App has necessary permissions
  - See the "GitHub App Config" section of this document

### Further steps
Replace `ls` with a linter command and use reviewdog to provide feedback on the results.

## GitHub App Config
While it's possible to share a single GitHub App between orgu-front and orgu-runner, it's recommended to use separate GitHub Apps for orgu-front and each orgu-runner job.

### orgu-front
Repository permissions:

- R/W: Checks
- R: Contents, Custom properties, Metadata, Pull requests

Webhook event subscription:

- Pull request

Note: Necessary `check_suite` and `check_run` events are automatically sent to the Webhook endpoint without subscription.

### orgu-runner
This is baseline and should be modify based on atual use cases.

Repository permissions:

- R/W: Checks
- R: Contents, Custom properties, Metadata, Pull requests

Webhook event subscription:

None, but set the same URL as orgu-front to enable the re-run checks feature.
When a user clicks the "re-run" button in the Checks UI, GitHub sends `check_suite.rerequested` or `check_run.rerequested` events to the URL.

For more details, see [docs/re-run.md](docs/re-run.md).

## Running orgu without AWS Lambda
### orgu-front server
Without AWS Lambda, orgu-front runs as a standalone HTTP server. It uses `POST /github/events` for incoming GitHub Webhook events. Deploy the orgu-front server in an environment such as Kubernetes and route GitHub Webhook events to this endpoint.

### Event queue relay
When running in an AWS Lambda environment, orgu-front uses Amazon EventBridge Event Bus as its event queue service. This setup can be made flexible by building a relay server that sends events to a queue-like service, which then fans out the events. Additionally, a custom queue receiver is required to trigger orgu-runner.

```
"orgu-front server" --HTTP POST-->
  "custom local HTTP relay server" --> "custom queue" -->
  "custom queue receiver" --HTTP POST--> "orgu-runner server"
```

For example, you can build a local relay service that sends events to Amazon Simple Notification Service (SNS), which then fans out the events to Amazon Simple Queue Service (SQS). A custom SQS receiver retrieves messages from SQS and sends them to the orgu-runner server.

orgu-front server sends to local orgu-runner server by default. This lacks "event fan-out" parts but one can get insight from that.

To make orgu-front uses custom event queue relay server, use `--event-queue-relay-endpoint` option. For more detail, see `orgu front server --help`.

See `CheckRequest` struct for event structure between orgu-front server and orgu-runner server.

### orgu-runner server
Without Amazon EventBridge Event Bus and AWS Lambda, orgu-runner operates as an HTTP server and processes received events. In server mode, orgu-runner uses `POST /run` to receive events.

## Operation
### Configuration
See `--help`. All operational configurations should be configurable via environment variables.

### Computing resources
Essentially, orgu-front requires fewer computing resources. In contrast, the resource consumption of orgu-runner significantly increases depending on its associated CI job. If the job involves CPU-intensive tasks, the container or pod must be allocated substantial computing resources.

### Job failures
orgu's approach to handling job failures is outlined as follows:

- **Job Execution Failure**: If a job is executed but fails, this does not cause orgu-runner to fail, as the responsibility lies with the user. orgu communicates the failure via the GitHub Checks API, notifying developers of the issue.
- **Job Execution Error**: Failure to execute a job results in an orgu-runner failure, typically due to misconfiguration. Most computing platforms can issue alerts for such failures. orgu also attempts to report these failures via the GitHub Checks API.
- **Job Timeout**: A job that executes successfully but exceeds its time limit does not cause orgu-runner to fail. This situation is reported through the GitHub Checks API.
- **Repository Checkout Timeout**: If the checkout of the target repository times out before job execution, it does not result in an orgu-runner failure. This event is also reported via the GitHub Checks API.

### Log level
Server: Use `RUST_LOG` environment to change default log level.

To see trace logs from orgu:

```
RUST_LOG=info,orgu=trace
```

To see debug logs from all targets:

```
RUST_LOG=debug
```

CLI uses: Use `-v` or `-vv` flag.

### Backtrace
Use `RUST_BACKTRACE=1` to see backtrace.

## Development
### Release
1. Update version of `Cargo.toml` and re-generate lock file
1. Git commit-push then create a PR and merge
1. Create a git tag with `git tag "$(cargo metadata --no-deps --format-version 1 | jq -r '"v" + .packages[0].version')"`
1. Push the git tag and wait the CI creates a GitHub Release and upload artifacts
1. Run `GITHUB_REF="refs/tags/$(cargo metadata --no-deps --format-version 1 | jq -r '"v" + .packages[0].version')" TARGET=orgu .github/scripts/update_formula` to update Homebrew formula file
1. Create a PR and merge
