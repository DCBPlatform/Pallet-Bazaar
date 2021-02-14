#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{
	decl_error, 
	decl_event, 
	decl_module, 
	decl_storage,
	ensure,  
	dispatch::{
		DispatchError, 
		DispatchResult
	},
	traits::{
		Currency, 
		ReservableCurrency,  
		Imbalance, 
		OnUnbalanced,	
		ExistenceRequirement::AllowDeath
	}
};
use frame_system::{
	self as system, 
	ensure_signed,
	ensure_root,
};
use sp_runtime::{ModuleId, traits::AccountIdConversion};

use parity_scale_codec::{
	Decode, 
	Encode
};
use sp_std::prelude::*;

const PALLET_ID: ModuleId = ModuleId(*b"8BAZAAR8");


pub trait Trait: frame_system::Trait {
	type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
	type Currency: ReservableCurrency<Self::AccountId>;
}

pub type BazaarTraderIndex = u128;
pub type BazaarTradeIndex = u128;

type AccountIdOf<T> = <T as system::Trait>::AccountId;
type BalanceOf<T> = <<T as Trait>::Currency as Currency<AccountIdOf<T>>>::Balance;

type BazaarTraderInfoOf<T> = BazaarTraderInfo<AccountIdOf<T>, BalanceOf<T>, <T as system::Trait>::BlockNumber>;
type BazaarTradeInfoOf<T> = BazaarTradeInfo<AccountIdOf<T>, BalanceOf<T>, <T as system::Trait>::BlockNumber>;

#[derive(Encode, Decode, Default, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct BazaarTraderInfo<AccountId, Balance, BlockNumber> {
	name: Vec<u8>,
	headline: Vec<u8>, 
	country: u8, 
	method: Vec<u8>, 
	ask_price: u128, // price that trader will sell at
	ask_limit: Balance,
	bid_price: u128, // price that trader will buy at
	bid_limit: Balance,
	account: AccountId,
	created: BlockNumber,
}

#[derive(Encode, Decode, Default, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct BazaarTradeInfo<AccountId, Balance, BlockNumber> {
	price: u128,
	amount: Balance,
	buyer: AccountId,
	seller: BazaarTraderIndex,
	escrowed: bool,
	received: bool,
	initiated: bool,
	created: BlockNumber,
}

decl_storage! {
	trait Store for Module<T: Trait> as TemplateModule {
		Something get(fn something): Option<u32>;

		pub BazaarTraderInformation get(fn bazaar_trader_info): 
			map hasher(blake2_128_concat) AccountIdOf<T> => BazaarTraderInfoOf<T>;	
		pub BazaarTraders get(fn bazaar_traders): 
			map hasher(blake2_128_concat) AccountIdOf<T> => BazaarTraderIndex;				
		pub BazaarTraderCount get(fn bazaar_trader_count): BazaarTraderIndex;		

		pub BazaarTrade get(fn bazaar_trade): 
			map hasher(blake2_128_concat) BazaarTradeIndex => BazaarTradeInfoOf<T>;		
		pub BazaarTradeCount get(fn bazaar_trade_count): BazaarTradeIndex;			
		pub BazaarTradeCountByTrader get(fn bazaar_trade_count_by_trader): 
			map hasher(blake2_128_concat) BazaarTraderIndex => BazaarTradeIndex;		
	}
}
decl_event!(
	pub enum Event<T> where 
	AccountId = <T as frame_system::Trait>::AccountId,
	Balance = BalanceOf<T>
	{
		// Buyer, Seller, Amount
		InitiatedBuy(AccountId, BazaarTraderIndex, Balance),
	}
);
decl_error! {
	pub enum Error for Module<T: Trait> {
		AlreadyTrader,
		NotAuthorisedAsTrader,

		NotBuyer,
		NotSeller,

		TradeAlreadyCompleted,
		TradeAlreadyEscrowed,
		TradeNotEscrowed,
		TradeLessThanOneDay,
	}
}
decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {

		type Error = Error<T>;
		fn deposit_event() = default;


		#[weight = 10_000]
		pub fn initiate_buy(origin, 
			price: u128, 
			amount: BalanceOf<T>, 
			seller: BazaarTraderIndex
		) -> DispatchResult {
			let buyer = ensure_signed(origin)?;

			let bazaar_trade_id = <BazaarTradeCount>::get();
			let created = <system::Module<T>>::block_number();

			<BazaarTrade<T>>::insert(bazaar_trade_id, BazaarTradeInfo {
				price,
				amount,
				buyer,
				seller,
				escrowed: false,
				received: false,
				initiated: true,
				created
			});

			Ok(())
		}

		#[weight = 10_000]
		pub fn escrow_coin(
			origin, 
			trade_id: BazaarTradeIndex
		) -> DispatchResult {

			let caller = ensure_signed(origin)?;
			let trader = <BazaarTraderInformation<T>>::get(&caller); 
			let trader_id = <BazaarTraders<T>>::get(caller.clone());
			let trade = <BazaarTrade<T>>::get(trade_id);
			ensure!(trader_id == trade.seller, Error::<T>::NotSeller);
			ensure!(trade.escrowed == false, Error::<T>::TradeAlreadyEscrowed);
			ensure!(trade.received == false, Error::<T>::TradeAlreadyCompleted);			

			T::Currency::transfer(&caller, &Self::account_id(), trade.amount, AllowDeath)
				.map_err(|_| DispatchError::Other("Cannot transfer escrow"))?;		
				
			<BazaarTrade<T>>::mutate(trade_id, |v| *v = BazaarTradeInfo {
				price: trade.price,
				amount: trade.amount,
				buyer: trade.buyer,
				seller: trade.seller,
				escrowed: true,
				received: trade.received,
				initiated: trade.initiated,
				created: trade.created
			});				

			Ok(())
		}

		#[weight = 10_000]
		pub fn cancel_escrow(
			origin, 
			trade_id: BazaarTradeIndex
		) -> DispatchResult {

			let caller = ensure_signed(origin)?;
			let trader = <BazaarTraderInformation<T>>::get(&caller); 
			let trader_id = <BazaarTraders<T>>::get(caller.clone());
			let trade = <BazaarTrade<T>>::get(trade_id);
			ensure!(trader_id == trade.seller, Error::<T>::NotSeller);
			ensure!(trade.escrowed == true, Error::<T>::TradeNotEscrowed);
			ensure!(trade.received == false, Error::<T>::TradeAlreadyCompleted);

			let current_block = <system::Module<T>>::block_number();
			let block_difference = current_block - trade.created; 
			ensure!(block_difference > 14400.into(), Error::<T>::TradeLessThanOneDay);	
			
			T::Currency::transfer(&Self::account_id(), &caller, trade.amount, AllowDeath)
				.map_err(|_| DispatchError::Other("Cannot transfer escrow"))?;			

			Ok(())
		}			

		#[weight = 10_000]
		pub fn confirm_received(
			origin, 
			trade_id: BazaarTradeIndex
		) -> DispatchResult {

			let caller = ensure_signed(origin)?;
			let trade = <BazaarTrade<T>>::get(trade_id);
			let trade_buyer = trade.buyer.clone();
			let trade_seller = trade.seller.clone();

			ensure!(caller == trade_buyer, Error::<T>::NotBuyer);
			ensure!(trade.escrowed == true, Error::<T>::TradeNotEscrowed);
			ensure!(trade.received == false, Error::<T>::TradeAlreadyCompleted);

			T::Currency::transfer( &Self::account_id(), &caller, trade.amount, AllowDeath)
				.map_err(|_| DispatchError::Other("Cannot transfer escrow"))?;	
		
			BazaarTradeCountByTrader::mutate(trade_seller,|v| *v -= 1);
			<BazaarTrade<T>>::mutate(trade_id, |v| *v = BazaarTradeInfo {
				price: trade.price,
				amount: trade.amount,
				buyer: trade.buyer,
				seller: trade.seller,
				escrowed: trade.escrowed,
				received: true,
				initiated: trade.initiated,
				created: trade.created
			});			
		

			Ok(())
		}		

		#[weight = 10_000]
		pub fn open_dispute(origin, trade_id: BazaarTradeIndex) -> DispatchResult {

			Ok(())
		}	
		
		#[weight = 10_000]
		pub fn close_dispute(
			origin, 
			trade_id: BazaarTradeIndex, 
			buyer_portion: u8, 
			seller_portion: u8
		) -> DispatchResult {

			Ok(())
		}
		
		#[weight = 10_000]
		pub fn register_trader(
			origin,
			name: Vec<u8>,
			headline: Vec<u8>,
			country: u8,
			method: Vec<u8>, 
			ask_price: u128,
			ask_limit: BalanceOf<T>,
			bid_price: u128,
			bid_limit: BalanceOf<T>,	
		) -> DispatchResult {	
	
			let new_trader = ensure_signed(origin)?;
			let trader_as_data = new_trader.clone();
			ensure!(!BazaarTraders::<T>::contains_key(&new_trader), Error::<T>::AlreadyTrader);

			let trader_count = BazaarTraderCount::get();
			let created = <system::Module<T>>::block_number();

			<BazaarTraderInformation<T>>::insert(new_trader, BazaarTraderInfo {
				name,
				headline,
				country,
				method,
				ask_price,
				ask_limit,
				bid_price,
				bid_limit,
				account: trader_as_data.clone(),
				created		
			});
			<BazaarTraders<T>>::insert(&trader_as_data, trader_count);
			BazaarTraderCount::put(trader_count + 1);

			Ok(())
		}	
		
		#[weight = 10_000]
		pub fn update_trader(
			origin,
			headline: Vec<u8>,
			method: Vec<u8>, 
		) -> DispatchResult {
	
			let who = ensure_signed(origin)?;
			ensure!(BazaarTraders::<T>::contains_key(&who), Error::<T>::NotAuthorisedAsTrader);
			let trader_as_data = who.clone();
			let trader = <BazaarTraderInformation<T>>::get(who);
			

			<BazaarTraderInformation<T>>::mutate(trader_as_data, |v| *v = BazaarTraderInfo {
				name: trader.name,
				headline: headline,
				country: trader.country,
				method: method,
				ask_price: trader.ask_price,
				ask_limit: trader.ask_limit,
				bid_price: trader.bid_price,
				bid_limit: trader.bid_limit,
				account: trader.account,
				created: trader.created
			});

			Ok(())
		}	
		
		#[weight = 10_000]
		pub fn update_trader_limit(
			origin,
			ask_price: u128,
			ask_limit: BalanceOf<T>,
			bid_price: u128,
			bid_limit: BalanceOf<T>,				
		) -> DispatchResult {
	
			let who = ensure_signed(origin)?;
			ensure!(BazaarTraders::<T>::contains_key(&who), Error::<T>::NotAuthorisedAsTrader);
			let trader_as_data = who.clone();
			let trader = <BazaarTraderInformation<T>>::get(who);

			<BazaarTraderInformation<T>>::mutate(trader_as_data, |v| *v = BazaarTraderInfo {
				name: trader.name,
				headline: trader.headline,
				country: trader.country,
				method: trader.method,
				ask_price: ask_price,
				ask_limit: ask_limit,
				bid_price: bid_price,
				bid_limit: bid_limit,
				account: trader.account,
				created: trader.created
			});

			Ok(())
		}		

	}
}

impl<T: Trait> Module<T> {
    pub fn account_id() -> T::AccountId {
        PALLET_ID.into_account()
    }

    pub fn balanace() -> BalanceOf<T> {
        T::Currency::free_balance(&Self::account_id())
    }
}
