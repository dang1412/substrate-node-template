#![cfg_attr(not(feature = "std"), no_std)]

/// Edit this file to define custom logic or remove it if it is not needed.
/// Learn more about FRAME and the core library of Substrate FRAME pallets:
/// <https://docs.substrate.io/v3/runtime/frame>
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

    use scale_info::TypeInfo;
    use integer_sqrt::IntegerSquareRoot;
    use sp_std::vec::Vec;

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

        #[pallet::constant]
        type MaxPixelOwned: Get<u32>;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

    // **************** STORAGE ****************
	// The pallet's runtime storage items.
	// https://docs.substrate.io/v3/runtime/storage

	#[pallet::storage]
	#[pallet::getter(fn pixel_cnt)]
    /// Keeps track of all minted Pixels.
	pub(super) type PixelCnt<T: Config> = StorageValue<_, u32, ValueQuery>;

    // Struct for holding Pixel information.
    #[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo)]
    #[scale_info(skip_type_params(T))]
    pub struct Pixel<T: Config> {
        pub id: u32,
        pub owner: T::AccountId,
        pub width: u32,
        pub height: u32,
        pub merged_to: Option<u32>,
    }

    impl<T: Config> MaxEncodedLen for Pixel<T> {
        fn max_encoded_len() -> usize {
            T::AccountId::max_encoded_len() * 2
        }
    }

    #[pallet::storage]
    #[pallet::getter(fn pixels)]
    pub(super) type Pixels<T: Config> = StorageMap<
        _,
        Twox64Concat,
        u32,
        Pixel<T>,
    >;

    #[pallet::storage]
    #[pallet::getter(fn pixels_owned)]
    /// Keeps track of what accounts own what Kitty.
    pub(super) type PixelsOwned<T: Config> = StorageMap<
        _,
        Twox64Concat,
        T::AccountId,
        BoundedVec<u32, T::MaxPixelOwned>,
        ValueQuery,
    >;

    // **************** ERROR ****************
    // Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {
        PixelCntOverflow,
        ExceedMaxPixelOwned,
        PixelNotExist,
        NotPixelOwner,
	}

    // **************** EVENT ****************
	// Pallets use events to inform users when important changes are made.
	// https://docs.substrate.io/v3/runtime/events-and-errors
	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Event documentation should end with an array that provides descriptive names for event
		/// parameters. [something, who]
        Minted(T::AccountId, u32),
		PixelMerged(u32, u32, u32),
	}

	// Dispatchable functions allows users to interact with the pallet and invoke state changes.
	// These functions materialize as "extrinsics", which are often compared to transactions.
	// Dispatchable functions must be annotated with a weight and must return a DispatchResult.
	#[pallet::call]
	impl<T: Config> Pallet<T> {

        #[pallet::weight(100)]
        pub fn mint_pixel(origin: OriginFor<T>, pixel_id: u32, width: u32, height: u32) -> DispatchResult {
            let sender = ensure_signed(origin)?; // <- add this line

            let mut rs = Ok(());

            // Logging to the console
            Self::iterate_pixel_area(pixel_id as i64, width as i64, height as i64, |id| -> bool {
                rs = Self::mint(id as u32, &sender); // <- add this line
                match rs {
                    Err(_) => false,    // break the loop
                    Ok(_) => true
                }
            });

            rs?;

            // ACTION #4: Deposit `Created` event
            Self::deposit_event(Event::Minted(sender, pixel_id));

            Ok(())
        }

        #[pallet::weight(100)]
        pub fn merge_pixel(origin: OriginFor<T>, pixel_id: u32, width: u32, height: u32) -> DispatchResult {
            let sender = ensure_signed(origin)?;
            let mut rs = Ok(());

            // TODO iterate and check all first before mutating storage

            let ids = Self::get_area_pixel_ids(pixel_id, width, height);

            // loop through each pixel, check and update
            for id in ids {
                let pixel = Self::pixels(id);

                // check pixel exist
                if pixel == None {
                    rs = Err(<Error<T>>::PixelNotExist);
                    break;
                }

                let mut pixel = pixel.unwrap();

                if id != pixel_id {
                    // check owner
                    if pixel.owner != sender {
                        rs = Err(<Error<T>>::NotPixelOwner);
                        break;
                    }
                    pixel.merged_to = Some(pixel_id);
                } else {
                    // top-left pixel
                    pixel.width = width;
                    pixel.height = height;
                }

                // update storage
                <Pixels<T>>::insert(id, pixel);
            }

            rs?;

            Ok(())
        }
	}

    //** Our helper functions.**//

    impl<T: Config> Pallet<T> {
        pub fn mint(
            pixel_id: u32,
            owner: &T::AccountId,
        ) -> Result<(), Error<T>> {
            // TODO check pixel_id valid

            let new_cnt = Self::pixel_cnt().checked_add(1)
                .ok_or(<Error<T>>::PixelCntOverflow)?;

            let pixel = Pixel::<T> {
                id: pixel_id,
                owner: owner.clone(),
                width: 1,
                height: 1,
                merged_to: None,
            };

            // Performs this operation first because as it may fail
            <PixelsOwned<T>>::try_mutate(owner, |pixel_vec| {
                pixel_vec.try_push(pixel_id)
            }).map_err(|_| <Error<T>>::ExceedMaxPixelOwned)?;

            <Pixels<T>>::insert(pixel_id, pixel);
            <PixelCnt<T>>::put(new_cnt);
            Ok(())
        }

        fn coord_degree(x: i64) -> i64 {
            let deg = if x >= 0 { x } else { - x - 1 };
            deg
        }

        fn max(x: i64, y: i64) -> i64 {
            if x > y { x } else { y }
        }

        pub fn get_coord(pixel_id: i64) -> (i64, i64) {
            let deg = pixel_id.integer_sqrt() / 2;

            let (edge, left) = {
                let edge_len = deg * 2 + 1;
                let count = pixel_id - i64::pow(deg * 2, 2);
                let edge = (count / edge_len) as i64;  // 0 1 2 3
                let left = count % edge_len;
        
                (edge, left)
            };
        
            match edge {
                // left edge
                0 => (- deg - 1, deg - 1 - left),
                // top edge
                1 => (- deg + left, - deg - 1),
                // right edge
                2 => (deg, - deg + left),
                // bottom edge
                3 => (deg - 1 - left, deg),
                // default (not run into)
                _ => (0, 0),
            }
        }
        
        pub fn get_pixel_id(x: i64, y: i64) -> i64 {
            let deg = Self::max(Self::coord_degree(x), Self::coord_degree(y));
            let edge_len = deg * 2 + 1;
        
            let min_coord = - deg - 1;
            let max_coord = deg;
        
            let mut index = i64::pow(deg * 2, 2);
            if x == min_coord && y < max_coord {
                index += (max_coord - 1) - y;
            } else if y == min_coord {
                index += edge_len + x - (min_coord + 1);
            } else if x == max_coord {
                index += edge_len * 2 + y - (min_coord + 1);
            } else {
                index += edge_len * 3 + (max_coord - 1) - x;
            }
        
            index
        }

        pub fn get_area_pixel_ids(pixel_id: u32, width: u32, height: u32) -> Vec<u32> {
            let (start_x, start_y) = Self::get_coord(pixel_id as i64);

            let mut ids: Vec<u32> = Vec::new();
            for i in 0..(width as i64) {
                for j in 0..(height as i64) {
                    let x = start_x + i;
                    let y = start_y + j;
                    let sub_pixel_id = Self::get_pixel_id(x, y);
                    ids.push(sub_pixel_id as u32)
                }
            }

            ids
        }
        
        pub fn iterate_pixel_area<F: FnMut(i64) -> bool>(pixel_id: i64, width: i64, height: i64, mut func: F) {
            let (start_x, start_y) = Self::get_coord(pixel_id.clone());
        
            for i in 0..(width as i64) {
                let mut is_continue = true;
                for j in 0..(height as i64) {
                    let x = start_x + i;
                    let y = start_y + j;
                    let sub_pixel_id = Self::get_pixel_id(x, y);
                    is_continue = func(sub_pixel_id);
                    if is_continue == false {
                        break;
                    }
                }
                if is_continue == false {
                    break;
                }
            }
        }

    }
}
