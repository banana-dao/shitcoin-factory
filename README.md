### shitcoin factory: a contract that extends tokenfactory to add some necessary features

|                    |     tokenfactory    |         shitcoin factory         |
|:------------------:|:-------------------:|:--------------------------------:|
|    capped supply   |          no         |                yes               |
| admin transferable | yes, to any address |     yes, only to null address    |
|   can burn tokens  |   from any account  |         held in contract         |
|      metadata      |    admin address    |    symbol, current/max supply    |

todo: liquidity functions for token admin

### assetlist

an on chain repository for token metadata, designed to sync with off chain assetlists to automate listing across multiple platforms
