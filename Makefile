.PHONY: help list-networks $(shell grep -oE '^[a-zA-Z0-9_-]+:' Makefile | sed 's/://')

SHELL := /bin/bash
NETWORKS := devnet mainnet
POSITIONAL_MARKET_ACTIONS := createMarket createOracle upgradeMarket upgradeMarketParams editAssetConfig editOracleTolerance show
POSITIONAL_ID_ACTIONS := addEModeCategory
POSITIONAL_ID_ASSET_ACTIONS := addAssetToEMode
POSITIONAL_ORACLE_ACTIONS := addOracles
POSITIONAL_ADDRESS_ACTIONS := verifyMarket setAshSwap
SIMPLE_ACTIONS := deployPriceAggregator upgradePriceAggregator pauseAggregator unpauseAggregator \
                  deployTemplateMarket upgradeTemplateMarket \
                  deployController upgradeController pauseController unpauseController \
                  registerToken claimRevenue \
                  listMarkets upgradeAllMarkets listEModeCategories \
                  verifyController verifyPriceAggregator

# Help command
help:
	@echo "MultiversX Lending Protocol Makefile"
	@echo ""
	@echo "Usage:"
	@echo "  make [network] [command] [args]"
	@echo ""
	@echo "Networks:"
	@for network in $(NETWORKS); do \
		echo "  $$network"; \
	done
	@echo ""
	@echo "Commands:"
	@echo "  Build commands:"
	@echo "    build               - Build contracts reproducibly using Docker"
	@echo ""
	@echo "  Deployment flow (no args, in order):"
	@echo "    deployPriceAggregator - Deploy the price aggregator contract"
	@echo "    deployTemplateMarket  - Deploy the market template with EGLD configuration"
	@echo "    deployController      - Deploy the controller contract"
	@echo "    registerToken         - Register the account token NFT"
	@echo ""
	@echo "  Upgrade and maintenance commands (no args):"
	@echo "    upgradePriceAggregator   - Upgrade the price aggregator contract" 
	@echo "    upgradeTemplateMarket    - Upgrade the market template contract"
	@echo "    upgradeController        - Upgrade the controller contract"
	@echo "    pauseController          - Pause the controller contract"
	@echo "    unpauseController        - Unpause the controller contract"
	@echo "    upgradeAllMarkets        - Upgrade all markets"
	@echo "    pauseAggregator          - Pause the price aggregator"
	@echo "    unpauseAggregator        - Unpause the price aggregator"
	@echo "    addOracles               - Add oracles to the price aggregator"
	@echo "    listMarkets              - List all available markets"
	@echo "    listEModeCategories      - List all E-Mode categories"
	@echo "    claimRevenue             - Claim revenue from all markets"
	@echo "    claimRevenue <token1> ..  - Claim revenue from specified tokens (e.g. EGLD USDT)"
	@echo ""
	@echo "  Verification commands:"
	@echo "    verifyController              - Verify the controller contract"
	@echo "    verifyMarket <address>        - Verify a market contract at the specified address"
	@echo "    verifyPriceAggregator        - Verify the price aggregator contract"
	@echo ""
	@echo "  Commands with market name:"
	@for action in $(POSITIONAL_MARKET_ACTIONS); do \
		echo "    $$action <market_name>"; \
	done
	@echo "    upgradeMarket <market_name>        - Upgrade market code only"
	@echo "    upgradeMarketParams <market_name>  - Upgrade market parameters only"
	@echo ""
	@echo "  Commands with category ID:"
	@for action in $(POSITIONAL_ID_ACTIONS); do \
		echo "    $$action <category_id>"; \
	done
	@echo ""
	@echo "  Commands with category ID and asset name:"
	@for action in $(POSITIONAL_ID_ASSET_ACTIONS); do \
		echo "    $$action <category_id> <asset_name>"; \
	done
	@echo ""
	@echo "  Commands with oracle addresses:"
	@for action in $(POSITIONAL_ORACLE_ACTIONS); do \
		echo "    $$action <address1> [address2] [address3] ..."; \
	done
	@echo ""
	@echo "  Commands with address:"
	@for action in $(POSITIONAL_ADDRESS_ACTIONS); do \
		echo "    $$action <address>"; \
	done
	@echo ""
	@echo "Examples:"
	@echo "  make devnet deployPriceAggregator"
	@echo "  make devnet deployTemplateMarket"
	@echo "  make devnet deployController"
	@echo "  make devnet createMarket EGLD"
	@echo "  make devnet addEModeCategory 1"
	@echo "  make devnet addAssetToEMode 1 USDC"
	@echo "  make devnet show EGLD"
	@echo "  make devnet setAshSwap erd1..."
	@echo "  make devnet claimRevenue EGLD USDT"

# Define the networks as targets that accept a second argument
$(NETWORKS):
	@if [ -z "$(word 2,$(MAKECMDGOALS))" ]; then \
		echo "Please specify an action for network $@"; \
		echo "Run 'make help' for available commands"; \
		exit 1; \
	fi; \
	if echo "$(POSITIONAL_MARKET_ACTIONS)" | grep -w "$(word 2,$(MAKECMDGOALS))" > /dev/null; then \
		if [ -z "$(word 3,$(MAKECMDGOALS))" ] && [ -z "$(MARKET)" ]; then \
			echo "Error: Market name is required for $(word 2,$(MAKECMDGOALS))."; \
			echo "Usage: make $@ $(word 2,$(MAKECMDGOALS)) <market_name>"; \
			exit 1; \
		fi; \
		market="$(MARKET)"; \
		if [ -z "$$market" ]; then \
			market="$(word 3,$(MAKECMDGOALS))"; \
		fi; \
		NETWORK=$@ ./configs/script.sh $(word 2,$(MAKECMDGOALS)) $$market; \
	elif echo "$(POSITIONAL_ID_ACTIONS)" | grep -w "$(word 2,$(MAKECMDGOALS))" > /dev/null; then \
		if [ -z "$(word 3,$(MAKECMDGOALS))" ] && [ -z "$(ID)" ]; then \
			echo "Error: Category ID is required for $(word 2,$(MAKECMDGOALS))."; \
			echo "Usage: make $@ $(word 2,$(MAKECMDGOALS)) <category_id>"; \
			exit 1; \
		fi; \
		id="$(ID)"; \
		if [ -z "$$id" ]; then \
			id="$(word 3,$(MAKECMDGOALS))"; \
		fi; \
		NETWORK=$@ ./configs/script.sh $(word 2,$(MAKECMDGOALS)) $$id; \
	elif echo "$(POSITIONAL_ID_ASSET_ACTIONS)" | grep -w "$(word 2,$(MAKECMDGOALS))" > /dev/null; then \
		if [ -z "$(word 3,$(MAKECMDGOALS))" ] || [ -z "$(word 4,$(MAKECMDGOALS))" ]; then \
			echo "Error: Both category ID and asset name are required for $(word 2,$(MAKECMDGOALS))."; \
			echo "Usage: make $@ $(word 2,$(MAKECMDGOALS)) <category_id> <asset_name>"; \
			exit 1; \
		fi; \
		id="$(ID)"; \
		asset="$(ASSET)"; \
		if [ -z "$$id" ]; then \
			id="$(word 3,$(MAKECMDGOALS))"; \
		fi; \
		if [ -z "$$asset" ]; then \
			asset="$(word 4,$(MAKECMDGOALS))"; \
		fi; \
		NETWORK=$@ ./configs/script.sh $(word 2,$(MAKECMDGOALS)) $$id $$asset; \
	elif echo "$(POSITIONAL_ORACLE_ACTIONS)" | grep -w "$(word 2,$(MAKECMDGOALS))" > /dev/null; then \
		if [ -z "$(word 3,$(MAKECMDGOALS))" ]; then \
			echo "Error: At least one oracle address is required for $(word 2,$(MAKECMDGOALS))."; \
			echo "Usage: make $@ $(word 2,$(MAKECMDGOALS)) <address1> [address2] [address3] ..."; \
			exit 1; \
		fi; \
		NETWORK=$@ ./configs/script.sh $(word 2,$(MAKECMDGOALS)) $(wordlist 3,$(words $(MAKECMDGOALS)),$(MAKECMDGOALS)); \
	elif echo "$(POSITIONAL_ADDRESS_ACTIONS)" | grep -w "$(word 2,$(MAKECMDGOALS))" > /dev/null; then \
		if [ -z "$(word 3,$(MAKECMDGOALS))" ]; then \
			echo "Error: Address is required for $(word 2,$(MAKECMDGOALS))."; \
			echo "Usage: make $@ $(word 2,$(MAKECMDGOALS)) <address>"; \
			exit 1; \
		fi; \
		address="$(ADDRESS)"; \
		if [ -z "$$address" ]; then \
			address="$(word 3,$(MAKECMDGOALS))"; \
		fi; \
		NETWORK=$@ ./configs/script.sh $(word 2,$(MAKECMDGOALS)) $$address; \
	elif echo "$(SIMPLE_ACTIONS)" | grep -w "$(word 2,$(MAKECMDGOALS))" > /dev/null; then \
		if [ "$(word 2,$(MAKECMDGOALS))" = "listMarkets" ]; then \
			NETWORK=$@ ./configs/script.sh list; \
		elif [ "$(word 2,$(MAKECMDGOALS))" = "registerToken" ]; then \
			NETWORK=$@ ./configs/script.sh registerAccountToken; \
		elif [ "$(word 2,$(MAKECMDGOALS))" = "deployTemplateMarket" ]; then \
			NETWORK=$@ ./configs/script.sh deployMarketTemplate EGLD; \
		elif [ "$(word 2,$(MAKECMDGOALS))" = "upgradeTemplateMarket" ]; then \
			NETWORK=$@ ./configs/script.sh upgradeMarketTemplate EGLD; \
		elif [ "$(word 2,$(MAKECMDGOALS))" = "claimRevenue" ]; then \
			if [ -n "$(word 3,$(MAKECMDGOALS))" ]; then \
				NETWORK=$@ ./configs/script.sh claimRevenue $(wordlist 3,$(words $(MAKECMDGOALS)),$(MAKECMDGOALS)); \
			else \
				NETWORK=$@ ./configs/script.sh claimRevenue; \
			fi; \
		else \
			NETWORK=$@ ./configs/script.sh $(word 2,$(MAKECMDGOALS)); \
		fi; \
	else \
		echo "Unknown action: $(word 2,$(MAKECMDGOALS))"; \
		echo "Run 'make help' for available commands"; \
		exit 1; \
	fi

# Define all commands as empty targets to allow for the syntax make network command
$(SIMPLE_ACTIONS) $(POSITIONAL_MARKET_ACTIONS) $(POSITIONAL_ID_ACTIONS) $(POSITIONAL_ID_ASSET_ACTIONS) $(POSITIONAL_ORACLE_ACTIONS) $(POSITIONAL_ADDRESS_ACTIONS):
	@: # Do nothing, this is just to let make accept these as targets

# Make third and fourth words targets (for positional arguments)
%:
	@:

# Keep the old pattern rules for backward compatibility
# Pattern rule for actions that require a MARKET parameter
$(foreach network,$(NETWORKS),$(foreach action,$(POSITIONAL_MARKET_ACTIONS),$(network)-$(action))):
	@network=$$(echo $@ | cut -d'-' -f1); \
	action=$$(echo $@ | cut -d'-' -f2-); \
	if [ -z "$(MARKET)" ] && [ -z "$(word 3,$(MAKECMDGOALS))" ]; then \
		echo "Error: Market name is required. Usage: make $@ <market_name> or make $@ MARKET=<market_name>"; \
		exit 1; \
	fi; \
	market="$(MARKET)"; \
	if [ -z "$$market" ]; then \
		market="$(word 3,$(MAKECMDGOALS))"; \
	fi; \
	NETWORK=$$network ./configs/script.sh $$action $$market

# Pattern rule for actions that require an ID parameter
$(foreach network,$(NETWORKS),$(foreach action,$(POSITIONAL_ID_ACTIONS),$(network)-$(action))):
	@network=$$(echo $@ | cut -d'-' -f1); \
	action=$$(echo $@ | cut -d'-' -f2-); \
	if [ -z "$(ID)" ] && [ -z "$(word 3,$(MAKECMDGOALS))" ]; then \
		echo "Error: Category ID is required. Usage: make $@ <id> or make $@ ID=<id>"; \
		exit 1; \
	fi; \
	id="$(ID)"; \
	if [ -z "$$id" ]; then \
		id="$(word 3,$(MAKECMDGOALS))"; \
	fi; \
	NETWORK=$$network ./configs/script.sh $$action $$id

# Pattern rule for actions that require both ID and ASSET parameters
$(foreach network,$(NETWORKS),$(foreach action,$(POSITIONAL_ID_ASSET_ACTIONS),$(network)-$(action))):
	@network=$$(echo $@ | cut -d'-' -f1); \
	action=$$(echo $@ | cut -d'-' -f2-); \
	if [ -z "$(ID)" ] && [ -z "$(word 3,$(MAKECMDGOALS))" ] || [ -z "$(ASSET)" ] && [ -z "$(word 4,$(MAKECMDGOALS))" ]; then \
		echo "Error: Both ID and ASSET parameters are required. Usage: make $@ <id> <asset> or make $@ ID=<id> ASSET=<asset>"; \
		exit 1; \
	fi; \
	id="$(ID)"; \
	asset="$(ASSET)"; \
	if [ -z "$$id" ]; then \
		id="$(word 3,$(MAKECMDGOALS))"; \
	fi; \
	if [ -z "$$asset" ]; then \
		asset="$(word 4,$(MAKECMDGOALS))"; \
	fi; \
	NETWORK=$$network ./configs/script.sh $$action $$id $$asset

# Pattern rule for simple actions without parameters
$(foreach network,$(NETWORKS),$(foreach action,$(SIMPLE_ACTIONS),$(network)-$(action))):
	@network=$$(echo $@ | cut -d'-' -f1); \
	action=$$(echo $@ | cut -d'-' -f2-); \
	if [ "$$action" = "listMarkets" ]; then \
		NETWORK=$$network ./configs/script.sh list; \
	elif [ "$$action" = "registerToken" ]; then \
		NETWORK=$$network ./configs/script.sh registerAccountToken; \
	elif [ "$$action" = "deployTemplateMarket" ]; then \
		NETWORK=$$network ./configs/script.sh deployMarketTemplate EGLD; \
	elif [ "$$action" = "upgradeTemplateMarket" ]; then \
		NETWORK=$$network ./configs/script.sh upgradeMarketTemplate EGLD; \
	else \
		NETWORK=$$network ./configs/script.sh $$action; \
	fi

# List networks
list-networks:
	./configs/script.sh networks

# Reproducible build target
build:
	rm -rf ./output-docker
	mxpy contract reproducible-build --docker-image="multiversx/sdk-rust-contract-builder:v11.0.0"

# Add new targets for E-Mode and asset configuration functions

# E-Mode targets - devnet
devnet-add-emode-category:
	NETWORK=devnet ./configs/script.sh addEModeCategory $(ID)

devnet-add-asset-to-emode:
	NETWORK=devnet ./configs/script.sh addAssetToEMode $(ID) $(ASSET)

devnet-list-emode-categories:
	NETWORK=devnet ./configs/script.sh listEModeCategories

devnet-edit-asset-config:
	NETWORK=devnet ./configs/script.sh editAssetConfig $(MARKET)

devnet-edit-oracle-tolerance:
	NETWORK=devnet ./configs/script.sh editOracleTolerance $(MARKET)

# E-Mode targets - mainnet
mainnet-add-emode-category:
	NETWORK=mainnet ./configs/script.sh addEModeCategory $(ID)

mainnet-add-asset-to-emode:
	NETWORK=mainnet ./configs/script.sh addAssetToEMode $(ID) $(ASSET)

mainnet-list-emode-categories:
	NETWORK=mainnet ./configs/script.sh listEModeCategories

mainnet-edit-asset-config:
	NETWORK=mainnet ./configs/script.sh editAssetConfig $(MARKET)

mainnet-edit-oracle-tolerance:
	NETWORK=mainnet ./configs/script.sh editOracleTolerance $(MARKET)

# Add new targets for verification functions - devnet
devnet-verify-controller:
	NETWORK=devnet ./configs/script.sh verifyController

devnet-verify-market:
	@if [ -z "$(ADDRESS)" ]; then \
		echo "Error: Market address is required for verification"; \
		echo "Usage: make devnet-verify-market ADDRESS=<market_address>"; \
		exit 1; \
	fi
	NETWORK=devnet ./configs/script.sh verifyMarket $(ADDRESS)

devnet-verify-price-aggregator:
	NETWORK=devnet ./configs/script.sh verifyPriceAggregator

# Add new targets for verification functions - mainnet
mainnet-verify-controller:
	NETWORK=mainnet ./configs/script.sh verifyController

mainnet-verify-market:
	@if [ -z "$(ADDRESS)" ]; then \
		echo "Error: Market address is required for verification"; \
		echo "Usage: make mainnet-verify-market ADDRESS=<market_address>"; \
		exit 1; \
	fi
	NETWORK=mainnet ./configs/script.sh verifyMarket $(ADDRESS)

mainnet-verify-price-aggregator:
	NETWORK=mainnet ./configs/script.sh verifyPriceAggregator

# Add new targets for setAshSwap - devnet
devnet-setAshSwap:
	@if [ -z "$(ADDRESS)" ]; then \
		echo "Error: AshSwap address is required"; \
		echo "Usage: make devnet-setAshSwap ADDRESS=<ash_swap_address>"; \
		exit 1; \
	fi
	NETWORK=devnet ./configs/script.sh setAshSwap $(ADDRESS)

# Add new targets for setAshSwap - mainnet
mainnet-setAshSwap:
	@if [ -z "$(ADDRESS)" ]; then \
		echo "Error: AshSwap address is required"; \
		echo "Usage: make mainnet-setAshSwap ADDRESS=<ash_swap_address>"; \
		exit 1; \
	fi
	NETWORK=mainnet ./configs/script.sh setAshSwap $(ADDRESS) 