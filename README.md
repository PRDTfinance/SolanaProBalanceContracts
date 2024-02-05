Contract Summary:

This is a balance depositing contract for users. Users can deposit SOL or USDT into the contract. 
Then they add withdraw requests on a backend and only the operator sends these balances to the users.
Admin wallet can withdraw any amount of SOL or USDT to his wallet.

Deposit events emit an event so the backend can sync these and create balances on a centralized database accordingly.

users can not call withdraw or sendWithdraw functions. Their requests are handled off chain and handled by master.operator wallet

On contract creation, the deployer runs init_master to create master PDA. This PDA holds admin and operator wallets
The deployer then runs init_ata to create USDT ATA for master PDA.

Master PDA keeps the SOL balance. Master PDA ATA keeps the USDT balance.
