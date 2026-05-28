import * as anchor from "@anchor-lang/core";
import { Program } from "@anchor-lang/core";
import { NftStaking } from "../target/types/nft_staking";
import { SystemProgram } from "@solana/web3.js";
import { MPL_CORE_PROGRAM_ID } from "@metaplex-foundation/mpl-core";
import { ASSOCIATED_TOKEN_PROGRAM_ID, TOKEN_PROGRAM_ID, getAssociatedTokenAddressSync } from "@solana/spl-token";

const MILLISECONDS_IN_DAY = 24 * 60 * 60 * 1000;
const REWARDS_BPS = 10000; 
const FREEZE_PERIOD_DAYS = 7;
const TIME_TRAVEL_IN_DAY = 8;


describe("nft-staking", () => {
  // Configure the client to use the local cluster.
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.nftStaking as Program<NftStaking>;

  const collectionKeypair = anchor.web3.Keypair.generate();
  const NftKeypair = anchor.web3.Keypair.generate();

  const updateAuthority = anchor.web3.PublicKey.findProgramAddressSync(
    [Buffer.from("update_authority"), collectionKeypair.publicKey.toBuffer()],
    program.programId
  )[0];

  const config = anchor.web3.PublicKey.findProgramAddressSync(
    [Buffer.from("config"), collectionKeypair.publicKey.toBuffer()],
    program.programId
  )[0];

  const rewardsMint = anchor.web3.PublicKey.findProgramAddressSync(
    [Buffer.from("rewards_mint"), config.toBuffer()],
    program.programId
  )[0];


  async function advanceTime(params: {absoluteTimestamp?: number, absoluteSlot?: number , absoluteEpoch?: number}): Promise<void> {
    const response = await fetch(provider.connection.rpcEndpoint, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        jsonrpc: "2.0",
        id: 1,
        method: 'surfnet_timeTravel',
        params: [params],
      }),
    });
    const result = await response.json() as any;
    if (result.error) throw new Error(`Time travel failed: ${result.error.message}`);
    await new Promise(resolve => setTimeout(resolve, 1000));
  }

  it("Create a Collection!", async () => {
    const collectionName = "Test Collection";
    const collectionUri = "https://example.com/collection";

    const tx = await program.methods
      .createCollection(collectionName, collectionUri)
      .accountsPartial({
        payer: provider.wallet.publicKey,
        collection: collectionKeypair.publicKey,
        updateAuthority,
        systemProgram: SystemProgram.programId,
        mplCoreProgram: MPL_CORE_PROGRAM_ID,
      })
      .signers([collectionKeypair])
      .rpc();
      console.log("Create Collection Tx Signature:", tx);
      console.log("Collection Public Key:", collectionKeypair.publicKey.toBase58());
  });

  it("Mint an NFT!", async () => {
    const nftName = "Test NFT";
    const nftUri = "https://example.com/nft";

    const tx = await program.methods
      .mintAsset(nftName, nftUri)
      .accountsPartial({
        user: provider.wallet.publicKey,
        collection: collectionKeypair.publicKey,
        asset: NftKeypair.publicKey,
        updateAuthority,
        systemProgram: SystemProgram.programId,
        mplCoreProgram: MPL_CORE_PROGRAM_ID,
      })
      .signers([NftKeypair])
      .rpc();
    console.log("Mint NFT Tx Signature:", tx);
    console.log("NFT Public Key:", NftKeypair.publicKey.toBase58());
  });

  it("Init Config!", async () => {
    const tx = await program.methods.initialize(REWARDS_BPS, FREEZE_PERIOD_DAYS).accountsPartial({
      admin: provider.wallet.publicKey,
      collection: collectionKeypair.publicKey,
      updateAuthority,
      config,
      rewardsMint,
      systemProgram: SystemProgram.programId,
      tokenProgram: TOKEN_PROGRAM_ID,
    }).rpc();
    console.log("Initialize Config Tx Signature:", tx);
    console.log("Config Public Key:", config.toBase58());
    console.log("Rewards Mint Public Key:", rewardsMint.toBase58());
  });

  it("Stake NFT!", async () => {
    const tx = await program.methods.stake().accountsPartial({
      owner: provider.wallet.publicKey,
      collection: collectionKeypair.publicKey,
      asset: NftKeypair.publicKey,
      config,
      updateAuthority,
      mplCoreProgram: MPL_CORE_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
    }).rpc();
    console.log("Stake NFT Tx Signature:", tx);
  });

  it("Attempt to Claim Rewards before freeze period ends (should fail)", async () => {
    const userRewardsAta = getAssociatedTokenAddressSync(rewardsMint, provider.wallet.publicKey, false, TOKEN_PROGRAM_ID, ASSOCIATED_TOKEN_PROGRAM_ID);
    
    try {
      const tx = await program.methods.claimRewards().accountsPartial({
        owner: provider.wallet.publicKey,
        collection: collectionKeypair.publicKey,
        asset: NftKeypair.publicKey,
        config,
        rewardsMint,
        userRewardsAta,
        updateAuthority,
        mplCoreProgram: MPL_CORE_PROGRAM_ID,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
      }).rpc();
      throw new Error("Claim rewards succeeded before freeze period ended");
    } catch (error) {
      if (error instanceof anchor.AnchorError && error.error.errorCode.code === "FreezePeriodNotOver") {
        console.log("Expected failure when claiming rewards before freeze period ends:", error.message);
      } else {
        throw error;
      }
    }
  });

  it("Advance time beyond freeze period and Claim Rewards!", async () => {
    const currentTimestamp = Date.now();
    await advanceTime({ absoluteTimestamp: currentTimestamp + (TIME_TRAVEL_IN_DAY * MILLISECONDS_IN_DAY) });
    
    const userRewardsAta = getAssociatedTokenAddressSync(rewardsMint, provider.wallet.publicKey, false, TOKEN_PROGRAM_ID, ASSOCIATED_TOKEN_PROGRAM_ID);
    
    const tx = await program.methods.claimRewards().accountsPartial({
      owner: provider.wallet.publicKey,
      collection: collectionKeypair.publicKey,
      asset: NftKeypair.publicKey,
      config,
      rewardsMint,
      userRewardsAta,
      updateAuthority,
      mplCoreProgram: MPL_CORE_PROGRAM_ID,
      tokenProgram: TOKEN_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
      associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
    }).rpc();
    console.log("Claim Rewards Tx Signature:", tx);
    console.log("Successfully claimed rewards.");
    console.log("User rewards Balance:", (await provider.connection.getTokenAccountBalance(userRewardsAta)).value.uiAmount);
  });

  it("Unstake NFT!", async () => {
    const tx = await program.methods.unstake().accountsPartial({
      owner: provider.wallet.publicKey,
      collection: collectionKeypair.publicKey,
      asset: NftKeypair.publicKey,
      config,
      updateAuthority,
      mplCoreProgram: MPL_CORE_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
    }).rpc();
    console.log("Unstake NFT Tx Signature:", tx);
    console.log("Successfully unstaked NFT.");
  });
});