pub mod objdetect {
	//! # Object Detection
	//!    # Cascade Classifier for Object Detection
	//!
	//! The object detector described below has been initially proposed by Paul Viola [Viola01](https://docs.opencv.org/4.6.0/d0/de3/citelist.html#CITEREF_Viola01) and
	//! improved by Rainer Lienhart [Lienhart02](https://docs.opencv.org/4.6.0/d0/de3/citelist.html#CITEREF_Lienhart02) .
	//!
	//! First, a classifier (namely a *cascade of boosted classifiers working with haar-like features*) is
	//! trained with a few hundred sample views of a particular object (i.e., a face or a car), called
	//! positive examples, that are scaled to the same size (say, 20x20), and negative examples - arbitrary
	//! images of the same size.
	//!
	//! After a classifier is trained, it can be applied to a region of interest (of the same size as used
	//! during the training) in an input image. The classifier outputs a "1" if the region is likely to show
	//! the object (i.e., face/car), and "0" otherwise. To search for the object in the whole image one can
	//! move the search window across the image and check every location using the classifier. The
	//! classifier is designed so that it can be easily "resized" in order to be able to find the objects of
	//! interest at different sizes, which is more efficient than resizing the image itself. So, to find an
	//! object of an unknown size in the image the scan procedure should be done several times at different
	//! scales.
	//!
	//! The word "cascade" in the classifier name means that the resultant classifier consists of several
	//! simpler classifiers (*stages*) that are applied subsequently to a region of interest until at some
	//! stage the candidate is rejected or all the stages are passed. The word "boosted" means that the
	//! classifiers at every stage of the cascade are complex themselves and they are built out of basic
	//! classifiers using one of four different boosting techniques (weighted voting). Currently Discrete
	//! Adaboost, Real Adaboost, Gentle Adaboost and Logitboost are supported. The basic classifiers are
	//! decision-tree classifiers with at least 2 leaves. Haar-like features are the input to the basic
	//! classifiers, and are calculated as described below. The current algorithm uses the following
	//! Haar-like features:
	//!
	//! ![image](https://docs.opencv.org/4.6.0/haarfeatures.png)
	//!
	//! The feature used in a particular classifier is specified by its shape (1a, 2b etc.), position within
	//! the region of interest and the scale (this scale is not the same as the scale used at the detection
	//! stage, though these two scales are multiplied). For example, in the case of the third line feature
	//! (2c) the response is calculated as the difference between the sum of image pixels under the
	//! rectangle covering the whole feature (including the two white stripes and the black stripe in the
	//! middle) and the sum of the image pixels under the black stripe multiplied by 3 in order to
	//! compensate for the differences in the size of areas. The sums of pixel values over a rectangular
	//! regions are calculated rapidly using integral images (see below and the integral description).
	//!
	//! Check [tutorial_cascade_classifier] "the corresponding tutorial" for more details.
	//!
	//! The following reference is for the detection part only. There is a separate application called
	//! opencv_traincascade that can train a cascade of boosted classifiers from a set of samples.
	//!
	//!
	//! Note: In the new C++ interface it is also possible to use LBP (local binary pattern) features in
	//! addition to Haar-like features. .. [Viola01] Paul Viola and Michael J. Jones. Rapid Object Detection
	//! using a Boosted Cascade of Simple Features. IEEE CVPR, 2001. The paper is available online at
	//! <https://github.com/SvHey/thesis/blob/master/Literature/ObjectDetection/violaJones_CVPR2001.pdf>
	//!
	//!    # HOG (Histogram of Oriented Gradients) descriptor and object detector
	//!    # QRCode detection and encoding
	//!    # DNN-based face detection and recognition
	//! Check [tutorial_dnn_face] "the corresponding tutorial" for more details.
	//!    # Common functions and classes
	use crate::mod_prelude::*;
	use crate::{core, sys, types};
	pub mod prelude {
		pub use super::{BaseCascadeClassifierTrait, BaseCascadeClassifierTraitConst, BaseCascadeClassifier_MaskGeneratorTrait, BaseCascadeClassifier_MaskGeneratorTraitConst, CascadeClassifierTrait, CascadeClassifierTraitConst, DetectionBasedTrackerTrait, DetectionBasedTrackerTraitConst, DetectionBasedTracker_ExtObjectTrait, DetectionBasedTracker_ExtObjectTraitConst, DetectionBasedTracker_IDetectorTrait, DetectionBasedTracker_IDetectorTraitConst, DetectionBasedTracker_ParametersTrait, DetectionBasedTracker_ParametersTraitConst, DetectionROITrait, DetectionROITraitConst, FaceDetectorYNTrait, FaceDetectorYNTraitConst, FaceRecognizerSFTrait, FaceRecognizerSFTraitConst, HOGDescriptorTrait, HOGDescriptorTraitConst, QRCodeDetectorTrait, QRCodeDetectorTraitConst, QRCodeEncoderTrait, QRCodeEncoderTraitConst, SimilarRectsTrait, SimilarRectsTraitConst};
	}

	pub const CASCADE_DO_CANNY_PRUNING: i32 = 1;
	pub const CASCADE_DO_ROUGH_SEARCH: i32 = 8;
	pub const CASCADE_FIND_BIGGEST_OBJECT: i32 = 4;
	pub const CASCADE_SCALE_IMAGE: i32 = 2;
	pub const DetectionBasedTracker_DETECTED: i32 = 1;
	pub const DetectionBasedTracker_DETECTED_NOT_SHOWN_YET: i32 = 0;
	pub const DetectionBasedTracker_DETECTED_TEMPORARY_LOST: i32 = 2;
	pub const DetectionBasedTracker_WRONG_OBJECT: i32 = 3;
	pub const FaceRecognizerSF_FR_COSINE: i32 = 0;
	pub const FaceRecognizerSF_FR_NORM_L2: i32 = 1;
	/// Default nlevels value.
	pub const HOGDescriptor_DEFAULT_NLEVELS: i32 = 64;
	pub const HOGDescriptor_DESCR_FORMAT_COL_BY_COL: i32 = 0;
	pub const HOGDescriptor_DESCR_FORMAT_ROW_BY_ROW: i32 = 1;
	/// Default histogramNormType
	pub const HOGDescriptor_L2Hys: i32 = 0;
	pub const QRCodeEncoder_CORRECT_LEVEL_H: i32 = 3;
	pub const QRCodeEncoder_CORRECT_LEVEL_L: i32 = 0;
	pub const QRCodeEncoder_CORRECT_LEVEL_M: i32 = 1;
	pub const QRCodeEncoder_CORRECT_LEVEL_Q: i32 = 2;
	pub const QRCodeEncoder_ECI_UTF8: i32 = 26;
	pub const QRCodeEncoder_MODE_ALPHANUMERIC: i32 = 2;
	pub const QRCodeEncoder_MODE_AUTO: i32 = -1;
	pub const QRCodeEncoder_MODE_BYTE: i32 = 4;
	pub const QRCodeEncoder_MODE_ECI: i32 = 7;
	pub const QRCodeEncoder_MODE_KANJI: i32 = 8;
	pub const QRCodeEncoder_MODE_NUMERIC: i32 = 1;
	pub const QRCodeEncoder_MODE_STRUCTURED_APPEND: i32 = 3;
	#[repr(C)]
	#[derive(Copy, Clone, Debug, PartialEq, Eq)]
	pub enum DetectionBasedTracker_ObjectStatus {
		DETECTED_NOT_SHOWN_YET = 0,
		DETECTED = 1,
		DETECTED_TEMPORARY_LOST = 2,
		WRONG_OBJECT = 3,
	}

	impl TryFrom<i32> for DetectionBasedTracker_ObjectStatus {
		type Error = crate::Error;

		fn try_from(value: i32) -> Result<Self, Self::Error> {
			match value {
				0 => Ok(Self::DETECTED_NOT_SHOWN_YET),
				1 => Ok(Self::DETECTED),
				2 => Ok(Self::DETECTED_TEMPORARY_LOST),
				3 => Ok(Self::WRONG_OBJECT),
				_ => Err(crate::Error::new(crate::core::StsBadArg, format!("Value: {value} is not valid for enum: crate::objdetect::DetectionBasedTracker_ObjectStatus"))),
			}
		}
	}

	opencv_type_enum! { crate::objdetect::DetectionBasedTracker_ObjectStatus }

	/// Definition of distance used for calculating the distance between two face features
	#[repr(C)]
	#[derive(Copy, Clone, Debug, PartialEq, Eq)]
	pub enum FaceRecognizerSF_DisType {
		FR_COSINE = 0,
		FR_NORM_L2 = 1,
	}

	impl TryFrom<i32> for FaceRecognizerSF_DisType {
		type Error = crate::Error;

		fn try_from(value: i32) -> Result<Self, Self::Error> {
			match value {
				0 => Ok(Self::FR_COSINE),
				1 => Ok(Self::FR_NORM_L2),
				_ => Err(crate::Error::new(crate::core::StsBadArg, format!("Value: {value} is not valid for enum: crate::objdetect::FaceRecognizerSF_DisType"))),
			}
		}
	}

	opencv_type_enum! { crate::objdetect::FaceRecognizerSF_DisType }

	#[repr(C)]
	#[derive(Copy, Clone, Debug, PartialEq, Eq)]
	pub enum HOGDescriptor_DescriptorStorageFormat {
		DESCR_FORMAT_COL_BY_COL = 0,
		DESCR_FORMAT_ROW_BY_ROW = 1,
	}

	impl TryFrom<i32> for HOGDescriptor_DescriptorStorageFormat {
		type Error = crate::Error;

		fn try_from(value: i32) -> Result<Self, Self::Error> {
			match value {
				0 => Ok(Self::DESCR_FORMAT_COL_BY_COL),
				1 => Ok(Self::DESCR_FORMAT_ROW_BY_ROW),
				_ => Err(crate::Error::new(crate::core::StsBadArg, format!("Value: {value} is not valid for enum: crate::objdetect::HOGDescriptor_DescriptorStorageFormat"))),
			}
		}
	}

	opencv_type_enum! { crate::objdetect::HOGDescriptor_DescriptorStorageFormat }

	#[repr(C)]
	#[derive(Copy, Clone, Debug, PartialEq, Eq)]
	pub enum HOGDescriptor_HistogramNormType {
		/// Default histogramNormType
		L2Hys = 0,
	}

	impl TryFrom<i32> for HOGDescriptor_HistogramNormType {
		type Error = crate::Error;

		fn try_from(value: i32) -> Result<Self, Self::Error> {
			match value {
				0 => Ok(Self::L2Hys),
				_ => Err(crate::Error::new(crate::core::StsBadArg, format!("Value: {value} is not valid for enum: crate::objdetect::HOGDescriptor_HistogramNormType"))),
			}
		}
	}

	opencv_type_enum! { crate::objdetect::HOGDescriptor_HistogramNormType }

	#[repr(C)]
	#[derive(Copy, Clone, Debug, PartialEq, Eq)]
	pub enum QRCodeEncoder_CorrectionLevel {
		CORRECT_LEVEL_L = 0,
		CORRECT_LEVEL_M = 1,
		CORRECT_LEVEL_Q = 2,
		CORRECT_LEVEL_H = 3,
	}

	impl TryFrom<i32> for QRCodeEncoder_CorrectionLevel {
		type Error = crate::Error;

		fn try_from(value: i32) -> Result<Self, Self::Error> {
			match value {
				0 => Ok(Self::CORRECT_LEVEL_L),
				1 => Ok(Self::CORRECT_LEVEL_M),
				2 => Ok(Self::CORRECT_LEVEL_Q),
				3 => Ok(Self::CORRECT_LEVEL_H),
				_ => Err(crate::Error::new(crate::core::StsBadArg, format!("Value: {value} is not valid for enum: crate::objdetect::QRCodeEncoder_CorrectionLevel"))),
			}
		}
	}

	opencv_type_enum! { crate::objdetect::QRCodeEncoder_CorrectionLevel }

	#[repr(C)]
	#[derive(Copy, Clone, Debug, PartialEq, Eq)]
	pub enum QRCodeEncoder_ECIEncodings {
		ECI_UTF8 = 26,
	}

	impl TryFrom<i32> for QRCodeEncoder_ECIEncodings {
		type Error = crate::Error;

		fn try_from(value: i32) -> Result<Self, Self::Error> {
			match value {
				26 => Ok(Self::ECI_UTF8),
				_ => Err(crate::Error::new(crate::core::StsBadArg, format!("Value: {value} is not valid for enum: crate::objdetect::QRCodeEncoder_ECIEncodings"))),
			}
		}
	}

	opencv_type_enum! { crate::objdetect::QRCodeEncoder_ECIEncodings }

	#[repr(C)]
	#[derive(Copy, Clone, Debug, PartialEq, Eq)]
	pub enum QRCodeEncoder_EncodeMode {
		MODE_AUTO = -1,
		MODE_NUMERIC = 1,
		MODE_ALPHANUMERIC = 2,
		MODE_BYTE = 4,
		MODE_ECI = 7,
		MODE_KANJI = 8,
		MODE_STRUCTURED_APPEND = 3,
	}

	impl TryFrom<i32> for QRCodeEncoder_EncodeMode {
		type Error = crate::Error;

		fn try_from(value: i32) -> Result<Self, Self::Error> {
			match value {
				-1 => Ok(Self::MODE_AUTO),
				1 => Ok(Self::MODE_NUMERIC),
				2 => Ok(Self::MODE_ALPHANUMERIC),
				4 => Ok(Self::MODE_BYTE),
				7 => Ok(Self::MODE_ECI),
				8 => Ok(Self::MODE_KANJI),
				3 => Ok(Self::MODE_STRUCTURED_APPEND),
				_ => Err(crate::Error::new(crate::core::StsBadArg, format!("Value: {value} is not valid for enum: crate::objdetect::QRCodeEncoder_EncodeMode"))),
			}
		}
	}

	opencv_type_enum! { crate::objdetect::QRCodeEncoder_EncodeMode }

	pub type DetectionBasedTracker_Object = core::Tuple<(core::Rect, i32)>;
	#[inline]
	pub fn create_face_detection_mask_generator() -> Result<core::Ptr<crate::objdetect::BaseCascadeClassifier_MaskGenerator>> {
		return_send!(via ocvrs_return);
		unsafe { sys::cv_createFaceDetectionMaskGenerator(ocvrs_return.as_mut_ptr()) };
		return_receive!(ocvrs_return => ret);
		let ret = ret.into_result()?;
		let ret = unsafe { core::Ptr::<crate::objdetect::BaseCascadeClassifier_MaskGenerator>::opencv_from_extern(ret) };
		Ok(ret)
	}

	/// @overload
	///
	/// ## Note
	/// This alternative version of [group_rectangles_meanshift] function uses the following default values for its arguments:
	/// * detect_threshold: 0.0
	/// * win_det_size: Size(64,128)
	#[inline]
	pub fn group_rectangles_meanshift_def(rect_list: &mut core::Vector<core::Rect>, found_weights: &mut core::Vector<f64>, found_scales: &mut core::Vector<f64>) -> Result<()> {
		return_send!(via ocvrs_return);
		unsafe { sys::cv_groupRectangles_meanshift_vectorLRectGR_vectorLdoubleGR_vectorLdoubleGR(rect_list.as_raw_mut_VectorOfRect(), found_weights.as_raw_mut_VectorOff64(), found_scales.as_raw_mut_VectorOff64(), ocvrs_return.as_mut_ptr()) };
		return_receive!(ocvrs_return => ret);
		let ret = ret.into_result()?;
		Ok(ret)
	}

	/// This is an overloaded member function, provided for convenience. It differs from the above function only in what argument(s) it accepts.
	///
	/// ## C++ default parameters
	/// * detect_threshold: 0.0
	/// * win_det_size: Size(64,128)
	#[inline]
	pub fn group_rectangles_meanshift(rect_list: &mut core::Vector<core::Rect>, found_weights: &mut core::Vector<f64>, found_scales: &mut core::Vector<f64>, detect_threshold: f64, win_det_size: core::Size) -> Result<()> {
		return_send!(via ocvrs_return);
		unsafe { sys::cv_groupRectangles_meanshift_vectorLRectGR_vectorLdoubleGR_vectorLdoubleGR_double_Size(rect_list.as_raw_mut_VectorOfRect(), found_weights.as_raw_mut_VectorOff64(), found_scales.as_raw_mut_VectorOff64(), detect_threshold, &win_det_size, ocvrs_return.as_mut_ptr()) };
		return_receive!(ocvrs_return => ret);
		let ret = ret.into_result()?;
		Ok(ret)
	}

	/// Groups the object candidate rectangles.
	///
	/// ## Parameters
	/// * rectList: Input/output vector of rectangles. Output vector includes retained and grouped
	/// rectangles. (The Python list is not modified in place.)
	/// * groupThreshold: Minimum possible number of rectangles minus 1. The threshold is used in a
	/// group of rectangles to retain it.
	/// * eps: Relative difference between sides of the rectangles to merge them into a group.
	///
	/// The function is a wrapper for the generic function partition . It clusters all the input rectangles
	/// using the rectangle equivalence criteria that combines rectangles with similar sizes and similar
	/// locations. The similarity is defined by eps. When eps=0 , no clustering is done at all. If
	/// ![inline formula](https://latex.codecogs.com/png.latex?%5Ctexttt%7Beps%7D%5Crightarrow%20%2B%5Cinf) , all the rectangles are put in one cluster. Then, the small
	/// clusters containing less than or equal to groupThreshold rectangles are rejected. In each other
	/// cluster, the average rectangle is computed and put into the output rectangle list.
	///
	/// ## Note
	/// This alternative version of [group_rectangles] function uses the following default values for its arguments:
	/// * eps: 0.2
	#[inline]
	pub fn group_rectangles_def(rect_list: &mut core::Vector<core::Rect>, group_threshold: i32) -> Result<()> {
		return_send!(via ocvrs_return);
		unsafe { sys::cv_groupRectangles_vectorLRectGR_int(rect_list.as_raw_mut_VectorOfRect(), group_threshold, ocvrs_return.as_mut_ptr()) };
		return_receive!(ocvrs_return => ret);
		let ret = ret.into_result()?;
		Ok(ret)
	}

	/// Groups the object candidate rectangles.
	///
	/// ## Parameters
	/// * rectList: Input/output vector of rectangles. Output vector includes retained and grouped
	/// rectangles. (The Python list is not modified in place.)
	/// * groupThreshold: Minimum possible number of rectangles minus 1. The threshold is used in a
	/// group of rectangles to retain it.
	/// * eps: Relative difference between sides of the rectangles to merge them into a group.
	///
	/// The function is a wrapper for the generic function partition . It clusters all the input rectangles
	/// using the rectangle equivalence criteria that combines rectangles with similar sizes and similar
	/// locations. The similarity is defined by eps. When eps=0 , no clustering is done at all. If
	/// ![inline formula](https://latex.codecogs.com/png.latex?%5Ctexttt%7Beps%7D%5Crightarrow%20%2B%5Cinf) , all the rectangles are put in one cluster. Then, the small
	/// clusters containing less than or equal to groupThreshold rectangles are rejected. In each other
	/// cluster, the average rectangle is computed and put into the output rectangle list.
	///
	/// ## C++ default parameters
	/// * eps: 0.2
	#[inline]
	pub fn group_rectangles(rect_list: &mut core::Vector<core::Rect>, group_threshold: i32, eps: f64) -> Result<()> {
		return_send!(via ocvrs_return);
		unsafe { sys::cv_groupRectangles_vectorLRectGR_int_double(rect_list.as_raw_mut_VectorOfRect(), group_threshold, eps, ocvrs_return.as_mut_ptr()) };
		return_receive!(ocvrs_return => ret);
		let ret = ret.into_result()?;
		Ok(ret)
	}

	/// Groups the object candidate rectangles.
	///
	/// ## Parameters
	/// * rectList: Input/output vector of rectangles. Output vector includes retained and grouped
	/// rectangles. (The Python list is not modified in place.)
	/// * groupThreshold: Minimum possible number of rectangles minus 1. The threshold is used in a
	/// group of rectangles to retain it.
	/// * eps: Relative difference between sides of the rectangles to merge them into a group.
	///
	/// The function is a wrapper for the generic function partition . It clusters all the input rectangles
	/// using the rectangle equivalence criteria that combines rectangles with similar sizes and similar
	/// locations. The similarity is defined by eps. When eps=0 , no clustering is done at all. If
	/// ![inline formula](https://latex.codecogs.com/png.latex?%5Ctexttt%7Beps%7D%5Crightarrow%20%2B%5Cinf) , all the rectangles are put in one cluster. Then, the small
	/// clusters containing less than or equal to groupThreshold rectangles are rejected. In each other
	/// cluster, the average rectangle is computed and put into the output rectangle list.
	///
	/// ## Overloaded parameters
	#[inline]
	pub fn group_rectangles_levelweights(rect_list: &mut core::Vector<core::Rect>, group_threshold: i32, eps: f64, weights: &mut core::Vector<i32>, level_weights: &mut core::Vector<f64>) -> Result<()> {
		return_send!(via ocvrs_return);
		unsafe { sys::cv_groupRectangles_vectorLRectGR_int_double_vectorLintGX_vectorLdoubleGX(rect_list.as_raw_mut_VectorOfRect(), group_threshold, eps, weights.as_raw_mut_VectorOfi32(), level_weights.as_raw_mut_VectorOff64(), ocvrs_return.as_mut_ptr()) };
		return_receive!(ocvrs_return => ret);
		let ret = ret.into_result()?;
		Ok(ret)
	}

	/// @overload
	///
	/// ## Note
	/// This alternative version of [group_rectangles_weights] function uses the following default values for its arguments:
	/// * eps: 0.2
	#[inline]
	pub fn group_rectangles_weights_def(rect_list: &mut core::Vector<core::Rect>, weights: &mut core::Vector<i32>, group_threshold: i32) -> Result<()> {
		return_send!(via ocvrs_return);
		unsafe { sys::cv_groupRectangles_vectorLRectGR_vectorLintGR_int(rect_list.as_raw_mut_VectorOfRect(), weights.as_raw_mut_VectorOfi32(), group_threshold, ocvrs_return.as_mut_ptr()) };
		return_receive!(ocvrs_return => ret);
		let ret = ret.into_result()?;
		Ok(ret)
	}

	/// Groups the object candidate rectangles.
	///
	/// ## Parameters
	/// * rectList: Input/output vector of rectangles. Output vector includes retained and grouped
	/// rectangles. (The Python list is not modified in place.)
	/// * groupThreshold: Minimum possible number of rectangles minus 1. The threshold is used in a
	/// group of rectangles to retain it.
	/// * eps: Relative difference between sides of the rectangles to merge them into a group.
	///
	/// The function is a wrapper for the generic function partition . It clusters all the input rectangles
	/// using the rectangle equivalence criteria that combines rectangles with similar sizes and similar
	/// locations. The similarity is defined by eps. When eps=0 , no clustering is done at all. If
	/// ![inline formula](https://latex.codecogs.com/png.latex?%5Ctexttt%7Beps%7D%5Crightarrow%20%2B%5Cinf) , all the rectangles are put in one cluster. Then, the small
	/// clusters containing less than or equal to groupThreshold rectangles are rejected. In each other
	/// cluster, the average rectangle is computed and put into the output rectangle list.
	///
	/// ## Overloaded parameters
	///
	/// ## C++ default parameters
	/// * eps: 0.2
	#[inline]
	pub fn group_rectangles_weights(rect_list: &mut core::Vector<core::Rect>, weights: &mut core::Vector<i32>, group_threshold: i32, eps: f64) -> Result<()> {
		return_send!(via ocvrs_return);
		unsafe { sys::cv_groupRectangles_vectorLRectGR_vectorLintGR_int_double(rect_list.as_raw_mut_VectorOfRect(), weights.as_raw_mut_VectorOfi32(), group_threshold, eps, ocvrs_return.as_mut_ptr()) };
		return_receive!(ocvrs_return => ret);
		let ret = ret.into_result()?;
		Ok(ret)
	}

	/// @overload
	///
	/// ## Note
	/// This alternative version of [group_rectangles_levels] function uses the following default values for its arguments:
	/// * eps: 0.2
	#[inline]
	pub fn group_rectangles_levels_def(rect_list: &mut core::Vector<core::Rect>, reject_levels: &mut core::Vector<i32>, level_weights: &mut core::Vector<f64>, group_threshold: i32) -> Result<()> {
		return_send!(via ocvrs_return);
		unsafe { sys::cv_groupRectangles_vectorLRectGR_vectorLintGR_vectorLdoubleGR_int(rect_list.as_raw_mut_VectorOfRect(), reject_levels.as_raw_mut_VectorOfi32(), level_weights.as_raw_mut_VectorOff64(), group_threshold, ocvrs_return.as_mut_ptr()) };
		return_receive!(ocvrs_return => ret);
		let ret = ret.into_result()?;
		Ok(ret)
	}

	/// Groups the object candidate rectangles.
	///
	/// ## Parameters
	/// * rectList: Input/output vector of rectangles. Output vector includes retained and grouped
	/// rectangles. (The Python list is not modified in place.)
	/// * groupThreshold: Minimum possible number of rectangles minus 1. The threshold is used in a
	/// group of rectangles to retain it.
	/// * eps: Relative difference between sides of the rectangles to merge them into a group.
	///
	/// The function is a wrapper for the generic function partition . It clusters all the input rectangles
	/// using the rectangle equivalence criteria that combines rectangles with similar sizes and similar
	/// locations. The similarity is defined by eps. When eps=0 , no clustering is done at all. If
	/// ![inline formula](https://latex.codecogs.com/png.latex?%5Ctexttt%7Beps%7D%5Crightarrow%20%2B%5Cinf) , all the rectangles are put in one cluster. Then, the small
	/// clusters containing less than or equal to groupThreshold rectangles are rejected. In each other
	/// cluster, the average rectangle is computed and put into the output rectangle list.
	///
	/// ## Overloaded parameters
	///
	/// ## C++ default parameters
	/// * eps: 0.2
	#[inline]
	pub fn group_rectangles_levels(rect_list: &mut core::Vector<core::Rect>, reject_levels: &mut core::Vector<i32>, level_weights: &mut core::Vector<f64>, group_threshold: i32, eps: f64) -> Result<()> {
		return_send!(via ocvrs_return);
		unsafe { sys::cv_groupRectangles_vectorLRectGR_vectorLintGR_vectorLdoubleGR_int_double(rect_list.as_raw_mut_VectorOfRect(), reject_levels.as_raw_mut_VectorOfi32(), level_weights.as_raw_mut_VectorOff64(), group_threshold, eps, ocvrs_return.as_mut_ptr()) };
		return_receive!(ocvrs_return => ret);
		let ret = ret.into_result()?;
		Ok(ret)
	}

	pub struct BaseCascadeClassifier {
		ptr: *mut c_void,
	}

	opencv_type_boxed! { BaseCascadeClassifier }

	impl Drop for BaseCascadeClassifier {
		#[inline]
		fn drop(&mut self) {
			unsafe { sys::cv_BaseCascadeClassifier_delete(self.as_raw_mut_BaseCascadeClassifier()) };
		}
	}

	unsafe impl Send for BaseCascadeClassifier {}

	/// Constant methods for [crate::objdetect::BaseCascadeClassifier]
	pub trait BaseCascadeClassifierTraitConst: core::AlgorithmTraitConst {
		fn as_raw_BaseCascadeClassifier(&self) -> *const c_void;

		#[inline]
		fn empty(&self) -> Result<bool> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_BaseCascadeClassifier_empty_const(self.as_raw_BaseCascadeClassifier(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		#[inline]
		fn is_old_format_cascade(&self) -> Result<bool> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_BaseCascadeClassifier_isOldFormatCascade_const(self.as_raw_BaseCascadeClassifier(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		#[inline]
		fn get_original_window_size(&self) -> Result<core::Size> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_BaseCascadeClassifier_getOriginalWindowSize_const(self.as_raw_BaseCascadeClassifier(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		#[inline]
		fn get_feature_type(&self) -> Result<i32> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_BaseCascadeClassifier_getFeatureType_const(self.as_raw_BaseCascadeClassifier(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

	}

	/// Mutable methods for [crate::objdetect::BaseCascadeClassifier]
	pub trait BaseCascadeClassifierTrait: core::AlgorithmTrait + crate::objdetect::BaseCascadeClassifierTraitConst {
		fn as_raw_mut_BaseCascadeClassifier(&mut self) -> *mut c_void;

		#[inline]
		fn load(&mut self, filename: &str) -> Result<bool> {
			extern_container_arg!(filename);
			return_send!(via ocvrs_return);
			unsafe { sys::cv_BaseCascadeClassifier_load_const_StringR(self.as_raw_mut_BaseCascadeClassifier(), filename.opencv_as_extern(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		#[inline]
		fn detect_multi_scale(&mut self, image: &impl ToInputArray, objects: &mut core::Vector<core::Rect>, scale_factor: f64, min_neighbors: i32, flags: i32, min_size: core::Size, max_size: core::Size) -> Result<()> {
			input_array_arg!(image);
			return_send!(via ocvrs_return);
			unsafe { sys::cv_BaseCascadeClassifier_detectMultiScale_const__InputArrayR_vectorLRectGR_double_int_int_Size_Size(self.as_raw_mut_BaseCascadeClassifier(), image.as_raw__InputArray(), objects.as_raw_mut_VectorOfRect(), scale_factor, min_neighbors, flags, &min_size, &max_size, ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		#[inline]
		fn detect_multi_scale_num(&mut self, image: &impl ToInputArray, objects: &mut core::Vector<core::Rect>, num_detections: &mut core::Vector<i32>, scale_factor: f64, min_neighbors: i32, flags: i32, min_size: core::Size, max_size: core::Size) -> Result<()> {
			input_array_arg!(image);
			return_send!(via ocvrs_return);
			unsafe { sys::cv_BaseCascadeClassifier_detectMultiScale_const__InputArrayR_vectorLRectGR_vectorLintGR_double_int_int_Size_Size(self.as_raw_mut_BaseCascadeClassifier(), image.as_raw__InputArray(), objects.as_raw_mut_VectorOfRect(), num_detections.as_raw_mut_VectorOfi32(), scale_factor, min_neighbors, flags, &min_size, &max_size, ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		#[inline]
		fn detect_multi_scale_levels(&mut self, image: &impl ToInputArray, objects: &mut core::Vector<core::Rect>, reject_levels: &mut core::Vector<i32>, level_weights: &mut core::Vector<f64>, scale_factor: f64, min_neighbors: i32, flags: i32, min_size: core::Size, max_size: core::Size, output_reject_levels: bool) -> Result<()> {
			input_array_arg!(image);
			return_send!(via ocvrs_return);
			unsafe { sys::cv_BaseCascadeClassifier_detectMultiScale_const__InputArrayR_vectorLRectGR_vectorLintGR_vectorLdoubleGR_double_int_int_Size_Size_bool(self.as_raw_mut_BaseCascadeClassifier(), image.as_raw__InputArray(), objects.as_raw_mut_VectorOfRect(), reject_levels.as_raw_mut_VectorOfi32(), level_weights.as_raw_mut_VectorOff64(), scale_factor, min_neighbors, flags, &min_size, &max_size, output_reject_levels, ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		#[inline]
		fn get_old_cascade(&mut self) -> Result<*mut c_void> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_BaseCascadeClassifier_getOldCascade(self.as_raw_mut_BaseCascadeClassifier(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		#[inline]
		fn set_mask_generator(&mut self, mask_generator: &core::Ptr<crate::objdetect::BaseCascadeClassifier_MaskGenerator>) -> Result<()> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_BaseCascadeClassifier_setMaskGenerator_const_PtrLMaskGeneratorGR(self.as_raw_mut_BaseCascadeClassifier(), mask_generator.as_raw_PtrOfBaseCascadeClassifier_MaskGenerator(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		#[inline]
		fn get_mask_generator(&mut self) -> Result<core::Ptr<crate::objdetect::BaseCascadeClassifier_MaskGenerator>> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_BaseCascadeClassifier_getMaskGenerator(self.as_raw_mut_BaseCascadeClassifier(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			let ret = unsafe { core::Ptr::<crate::objdetect::BaseCascadeClassifier_MaskGenerator>::opencv_from_extern(ret) };
			Ok(ret)
		}

	}

	impl std::fmt::Debug for BaseCascadeClassifier {
		#[inline]
		fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
			f.debug_struct("BaseCascadeClassifier")
				.finish()
		}
	}

	boxed_cast_base! { BaseCascadeClassifier, core::Algorithm, cv_BaseCascadeClassifier_to_Algorithm }

	impl core::AlgorithmTraitConst for BaseCascadeClassifier {
		#[inline] fn as_raw_Algorithm(&self) -> *const c_void { self.as_raw() }
	}

	impl core::AlgorithmTrait for BaseCascadeClassifier {
		#[inline] fn as_raw_mut_Algorithm(&mut self) -> *mut c_void { self.as_raw_mut() }
	}

	boxed_ref! { BaseCascadeClassifier, core::AlgorithmTraitConst, as_raw_Algorithm, core::AlgorithmTrait, as_raw_mut_Algorithm }

	impl crate::objdetect::BaseCascadeClassifierTraitConst for BaseCascadeClassifier {
		#[inline] fn as_raw_BaseCascadeClassifier(&self) -> *const c_void { self.as_raw() }
	}

	impl crate::objdetect::BaseCascadeClassifierTrait for BaseCascadeClassifier {
		#[inline] fn as_raw_mut_BaseCascadeClassifier(&mut self) -> *mut c_void { self.as_raw_mut() }
	}

	boxed_ref! { BaseCascadeClassifier, crate::objdetect::BaseCascadeClassifierTraitConst, as_raw_BaseCascadeClassifier, crate::objdetect::BaseCascadeClassifierTrait, as_raw_mut_BaseCascadeClassifier }

	pub struct BaseCascadeClassifier_MaskGenerator {
		ptr: *mut c_void,
	}

	opencv_type_boxed! { BaseCascadeClassifier_MaskGenerator }

	impl Drop for BaseCascadeClassifier_MaskGenerator {
		#[inline]
		fn drop(&mut self) {
			unsafe { sys::cv_BaseCascadeClassifier_MaskGenerator_delete(self.as_raw_mut_BaseCascadeClassifier_MaskGenerator()) };
		}
	}

	unsafe impl Send for BaseCascadeClassifier_MaskGenerator {}

	/// Constant methods for [crate::objdetect::BaseCascadeClassifier_MaskGenerator]
	pub trait BaseCascadeClassifier_MaskGeneratorTraitConst {
		fn as_raw_BaseCascadeClassifier_MaskGenerator(&self) -> *const c_void;

	}

	/// Mutable methods for [crate::objdetect::BaseCascadeClassifier_MaskGenerator]
	pub trait BaseCascadeClassifier_MaskGeneratorTrait: crate::objdetect::BaseCascadeClassifier_MaskGeneratorTraitConst {
		fn as_raw_mut_BaseCascadeClassifier_MaskGenerator(&mut self) -> *mut c_void;

		#[inline]
		fn generate_mask(&mut self, src: &impl core::MatTraitConst) -> Result<core::Mat> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_BaseCascadeClassifier_MaskGenerator_generateMask_const_MatR(self.as_raw_mut_BaseCascadeClassifier_MaskGenerator(), src.as_raw_Mat(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			let ret = unsafe { core::Mat::opencv_from_extern(ret) };
			Ok(ret)
		}

		#[inline]
		fn initialize_mask(&mut self, unnamed: &impl core::MatTraitConst) -> Result<()> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_BaseCascadeClassifier_MaskGenerator_initializeMask_const_MatR(self.as_raw_mut_BaseCascadeClassifier_MaskGenerator(), unnamed.as_raw_Mat(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

	}

	impl std::fmt::Debug for BaseCascadeClassifier_MaskGenerator {
		#[inline]
		fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
			f.debug_struct("BaseCascadeClassifier_MaskGenerator")
				.finish()
		}
	}

	impl crate::objdetect::BaseCascadeClassifier_MaskGeneratorTraitConst for BaseCascadeClassifier_MaskGenerator {
		#[inline] fn as_raw_BaseCascadeClassifier_MaskGenerator(&self) -> *const c_void { self.as_raw() }
	}

	impl crate::objdetect::BaseCascadeClassifier_MaskGeneratorTrait for BaseCascadeClassifier_MaskGenerator {
		#[inline] fn as_raw_mut_BaseCascadeClassifier_MaskGenerator(&mut self) -> *mut c_void { self.as_raw_mut() }
	}

	boxed_ref! { BaseCascadeClassifier_MaskGenerator, crate::objdetect::BaseCascadeClassifier_MaskGeneratorTraitConst, as_raw_BaseCascadeClassifier_MaskGenerator, crate::objdetect::BaseCascadeClassifier_MaskGeneratorTrait, as_raw_mut_BaseCascadeClassifier_MaskGenerator }

	/// @example samples/cpp/facedetect.cpp
	/// This program demonstrates usage of the Cascade classifier class
	/// \image html Cascade_Classifier_Tutorial_Result_Haar.jpg "Sample screenshot" width=321 height=254
	///
	/// Cascade classifier class for object detection.
	pub struct CascadeClassifier {
		ptr: *mut c_void,
	}

	opencv_type_boxed! { CascadeClassifier }

	impl Drop for CascadeClassifier {
		#[inline]
		fn drop(&mut self) {
			unsafe { sys::cv_CascadeClassifier_delete(self.as_raw_mut_CascadeClassifier()) };
		}
	}

	unsafe impl Send for CascadeClassifier {}

	impl CascadeClassifier {
		#[inline]
		pub fn default() -> Result<crate::objdetect::CascadeClassifier> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_CascadeClassifier_CascadeClassifier(ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			let ret = unsafe { crate::objdetect::CascadeClassifier::opencv_from_extern(ret) };
			Ok(ret)
		}

		/// Loads a classifier from a file.
		///
		/// ## Parameters
		/// * filename: Name of the file from which the classifier is loaded.
		#[inline]
		pub fn new(filename: &str) -> Result<crate::objdetect::CascadeClassifier> {
			extern_container_arg!(filename);
			return_send!(via ocvrs_return);
			unsafe { sys::cv_CascadeClassifier_CascadeClassifier_const_StringR(filename.opencv_as_extern(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			let ret = unsafe { crate::objdetect::CascadeClassifier::opencv_from_extern(ret) };
			Ok(ret)
		}

		#[inline]
		pub fn convert(oldcascade: &str, newcascade: &str) -> Result<bool> {
			extern_container_arg!(oldcascade);
			extern_container_arg!(newcascade);
			return_send!(via ocvrs_return);
			unsafe { sys::cv_CascadeClassifier_convert_const_StringR_const_StringR(oldcascade.opencv_as_extern(), newcascade.opencv_as_extern(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

	}

	/// Constant methods for [crate::objdetect::CascadeClassifier]
	pub trait CascadeClassifierTraitConst {
		fn as_raw_CascadeClassifier(&self) -> *const c_void;

		/// Checks whether the classifier has been loaded.
		#[inline]
		fn empty(&self) -> Result<bool> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_CascadeClassifier_empty_const(self.as_raw_CascadeClassifier(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		#[inline]
		fn is_old_format_cascade(&self) -> Result<bool> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_CascadeClassifier_isOldFormatCascade_const(self.as_raw_CascadeClassifier(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		#[inline]
		fn get_original_window_size(&self) -> Result<core::Size> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_CascadeClassifier_getOriginalWindowSize_const(self.as_raw_CascadeClassifier(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		#[inline]
		fn get_feature_type(&self) -> Result<i32> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_CascadeClassifier_getFeatureType_const(self.as_raw_CascadeClassifier(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

	}

	/// Mutable methods for [crate::objdetect::CascadeClassifier]
	pub trait CascadeClassifierTrait: crate::objdetect::CascadeClassifierTraitConst {
		fn as_raw_mut_CascadeClassifier(&mut self) -> *mut c_void;

		#[inline]
		fn cc(&mut self) -> core::Ptr<crate::objdetect::BaseCascadeClassifier> {
			let ret = unsafe { sys::cv_CascadeClassifier_propCc(self.as_raw_mut_CascadeClassifier()) };
			let ret = unsafe { core::Ptr::<crate::objdetect::BaseCascadeClassifier>::opencv_from_extern(ret) };
			ret
		}

		#[inline]
		fn set_cc(&mut self, val: core::Ptr<crate::objdetect::BaseCascadeClassifier>) {
			let ret = unsafe { sys::cv_CascadeClassifier_propCc_const_PtrLBaseCascadeClassifierG(self.as_raw_mut_CascadeClassifier(), val.as_raw_PtrOfBaseCascadeClassifier()) };
			ret
		}

		/// Loads a classifier from a file.
		///
		/// ## Parameters
		/// * filename: Name of the file from which the classifier is loaded. The file may contain an old
		/// HAAR classifier trained by the haartraining application or a new cascade classifier trained by the
		/// traincascade application.
		#[inline]
		fn load(&mut self, filename: &str) -> Result<bool> {
			extern_container_arg!(filename);
			return_send!(via ocvrs_return);
			unsafe { sys::cv_CascadeClassifier_load_const_StringR(self.as_raw_mut_CascadeClassifier(), filename.opencv_as_extern(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		/// Reads a classifier from a FileStorage node.
		///
		///
		/// Note: The file may contain a new cascade classifier (trained by the traincascade application) only.
		#[inline]
		fn read(&mut self, node: &impl core::FileNodeTraitConst) -> Result<bool> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_CascadeClassifier_read_const_FileNodeR(self.as_raw_mut_CascadeClassifier(), node.as_raw_FileNode(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		/// Detects objects of different sizes in the input image. The detected objects are returned as a list
		/// of rectangles.
		///
		/// ## Parameters
		/// * image: Matrix of the type CV_8U containing an image where objects are detected.
		/// * objects: Vector of rectangles where each rectangle contains the detected object, the
		/// rectangles may be partially outside the original image.
		/// * scaleFactor: Parameter specifying how much the image size is reduced at each image scale.
		/// * minNeighbors: Parameter specifying how many neighbors each candidate rectangle should have
		/// to retain it.
		/// * flags: Parameter with the same meaning for an old cascade as in the function
		/// cvHaarDetectObjects. It is not used for a new cascade.
		/// * minSize: Minimum possible object size. Objects smaller than that are ignored.
		/// * maxSize: Maximum possible object size. Objects larger than that are ignored. If `maxSize == minSize` model is evaluated on single scale.
		///
		/// ## C++ default parameters
		/// * scale_factor: 1.1
		/// * min_neighbors: 3
		/// * flags: 0
		/// * min_size: Size()
		/// * max_size: Size()
		#[inline]
		fn detect_multi_scale(&mut self, image: &impl ToInputArray, objects: &mut core::Vector<core::Rect>, scale_factor: f64, min_neighbors: i32, flags: i32, min_size: core::Size, max_size: core::Size) -> Result<()> {
			input_array_arg!(image);
			return_send!(via ocvrs_return);
			unsafe { sys::cv_CascadeClassifier_detectMultiScale_const__InputArrayR_vectorLRectGR_double_int_int_Size_Size(self.as_raw_mut_CascadeClassifier(), image.as_raw__InputArray(), objects.as_raw_mut_VectorOfRect(), scale_factor, min_neighbors, flags, &min_size, &max_size, ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		/// Detects objects of different sizes in the input image. The detected objects are returned as a list
		/// of rectangles.
		///
		/// ## Parameters
		/// * image: Matrix of the type CV_8U containing an image where objects are detected.
		/// * objects: Vector of rectangles where each rectangle contains the detected object, the
		/// rectangles may be partially outside the original image.
		/// * scaleFactor: Parameter specifying how much the image size is reduced at each image scale.
		/// * minNeighbors: Parameter specifying how many neighbors each candidate rectangle should have
		/// to retain it.
		/// * flags: Parameter with the same meaning for an old cascade as in the function
		/// cvHaarDetectObjects. It is not used for a new cascade.
		/// * minSize: Minimum possible object size. Objects smaller than that are ignored.
		/// * maxSize: Maximum possible object size. Objects larger than that are ignored. If `maxSize == minSize` model is evaluated on single scale.
		///
		/// ## Note
		/// This alternative version of [CascadeClassifierTrait::detect_multi_scale] function uses the following default values for its arguments:
		/// * scale_factor: 1.1
		/// * min_neighbors: 3
		/// * flags: 0
		/// * min_size: Size()
		/// * max_size: Size()
		#[inline]
		fn detect_multi_scale_def(&mut self, image: &impl ToInputArray, objects: &mut core::Vector<core::Rect>) -> Result<()> {
			input_array_arg!(image);
			return_send!(via ocvrs_return);
			unsafe { sys::cv_CascadeClassifier_detectMultiScale_const__InputArrayR_vectorLRectGR(self.as_raw_mut_CascadeClassifier(), image.as_raw__InputArray(), objects.as_raw_mut_VectorOfRect(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		/// Detects objects of different sizes in the input image. The detected objects are returned as a list
		/// of rectangles.
		///
		/// ## Parameters
		/// * image: Matrix of the type CV_8U containing an image where objects are detected.
		/// * objects: Vector of rectangles where each rectangle contains the detected object, the
		/// rectangles may be partially outside the original image.
		/// * scaleFactor: Parameter specifying how much the image size is reduced at each image scale.
		/// * minNeighbors: Parameter specifying how many neighbors each candidate rectangle should have
		/// to retain it.
		/// * flags: Parameter with the same meaning for an old cascade as in the function
		/// cvHaarDetectObjects. It is not used for a new cascade.
		/// * minSize: Minimum possible object size. Objects smaller than that are ignored.
		/// * maxSize: Maximum possible object size. Objects larger than that are ignored. If `maxSize == minSize` model is evaluated on single scale.
		///
		/// ## Overloaded parameters
		///
		/// * image: Matrix of the type CV_8U containing an image where objects are detected.
		/// * objects: Vector of rectangles where each rectangle contains the detected object, the
		/// rectangles may be partially outside the original image.
		/// * numDetections: Vector of detection numbers for the corresponding objects. An object's number
		/// of detections is the number of neighboring positively classified rectangles that were joined
		/// together to form the object.
		/// * scaleFactor: Parameter specifying how much the image size is reduced at each image scale.
		/// * minNeighbors: Parameter specifying how many neighbors each candidate rectangle should have
		/// to retain it.
		/// * flags: Parameter with the same meaning for an old cascade as in the function
		/// cvHaarDetectObjects. It is not used for a new cascade.
		/// * minSize: Minimum possible object size. Objects smaller than that are ignored.
		/// * maxSize: Maximum possible object size. Objects larger than that are ignored. If `maxSize == minSize` model is evaluated on single scale.
		///
		/// ## C++ default parameters
		/// * scale_factor: 1.1
		/// * min_neighbors: 3
		/// * flags: 0
		/// * min_size: Size()
		/// * max_size: Size()
		#[inline]
		fn detect_multi_scale2(&mut self, image: &impl ToInputArray, objects: &mut core::Vector<core::Rect>, num_detections: &mut core::Vector<i32>, scale_factor: f64, min_neighbors: i32, flags: i32, min_size: core::Size, max_size: core::Size) -> Result<()> {
			input_array_arg!(image);
			return_send!(via ocvrs_return);
			unsafe { sys::cv_CascadeClassifier_detectMultiScale_const__InputArrayR_vectorLRectGR_vectorLintGR_double_int_int_Size_Size(self.as_raw_mut_CascadeClassifier(), image.as_raw__InputArray(), objects.as_raw_mut_VectorOfRect(), num_detections.as_raw_mut_VectorOfi32(), scale_factor, min_neighbors, flags, &min_size, &max_size, ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		/// @overload
		/// ## Parameters
		/// * image: Matrix of the type CV_8U containing an image where objects are detected.
		/// * objects: Vector of rectangles where each rectangle contains the detected object, the
		/// rectangles may be partially outside the original image.
		/// * numDetections: Vector of detection numbers for the corresponding objects. An object's number
		/// of detections is the number of neighboring positively classified rectangles that were joined
		/// together to form the object.
		/// * scaleFactor: Parameter specifying how much the image size is reduced at each image scale.
		/// * minNeighbors: Parameter specifying how many neighbors each candidate rectangle should have
		/// to retain it.
		/// * flags: Parameter with the same meaning for an old cascade as in the function
		/// cvHaarDetectObjects. It is not used for a new cascade.
		/// * minSize: Minimum possible object size. Objects smaller than that are ignored.
		/// * maxSize: Maximum possible object size. Objects larger than that are ignored. If `maxSize == minSize` model is evaluated on single scale.
		///
		/// ## Note
		/// This alternative version of [CascadeClassifierTrait::detect_multi_scale2] function uses the following default values for its arguments:
		/// * scale_factor: 1.1
		/// * min_neighbors: 3
		/// * flags: 0
		/// * min_size: Size()
		/// * max_size: Size()
		#[inline]
		fn detect_multi_scale2_def(&mut self, image: &impl ToInputArray, objects: &mut core::Vector<core::Rect>, num_detections: &mut core::Vector<i32>) -> Result<()> {
			input_array_arg!(image);
			return_send!(via ocvrs_return);
			unsafe { sys::cv_CascadeClassifier_detectMultiScale_const__InputArrayR_vectorLRectGR_vectorLintGR(self.as_raw_mut_CascadeClassifier(), image.as_raw__InputArray(), objects.as_raw_mut_VectorOfRect(), num_detections.as_raw_mut_VectorOfi32(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		/// Detects objects of different sizes in the input image. The detected objects are returned as a list
		/// of rectangles.
		///
		/// ## Parameters
		/// * image: Matrix of the type CV_8U containing an image where objects are detected.
		/// * objects: Vector of rectangles where each rectangle contains the detected object, the
		/// rectangles may be partially outside the original image.
		/// * scaleFactor: Parameter specifying how much the image size is reduced at each image scale.
		/// * minNeighbors: Parameter specifying how many neighbors each candidate rectangle should have
		/// to retain it.
		/// * flags: Parameter with the same meaning for an old cascade as in the function
		/// cvHaarDetectObjects. It is not used for a new cascade.
		/// * minSize: Minimum possible object size. Objects smaller than that are ignored.
		/// * maxSize: Maximum possible object size. Objects larger than that are ignored. If `maxSize == minSize` model is evaluated on single scale.
		///
		/// ## Overloaded parameters
		///
		/// This function allows you to retrieve the final stage decision certainty of classification.
		/// For this, one needs to set `outputRejectLevels` on true and provide the `rejectLevels` and `levelWeights` parameter.
		/// For each resulting detection, `levelWeights` will then contain the certainty of classification at the final stage.
		/// This value can then be used to separate strong from weaker classifications.
		///
		/// A code sample on how to use it efficiently can be found below:
		/// ```C++
		/// Mat img;
		/// vector<double> weights;
		/// vector<int> levels;
		/// vector<Rect> detections;
		/// CascadeClassifier model("/path/to/your/model.xml");
		/// model.detectMultiScale(img, detections, levels, weights, 1.1, 3, 0, Size(), Size(), true);
		/// cerr << "Detection " << detections[0] << " with weight " << weights[0] << endl;
		/// ```
		///
		///
		/// ## C++ default parameters
		/// * scale_factor: 1.1
		/// * min_neighbors: 3
		/// * flags: 0
		/// * min_size: Size()
		/// * max_size: Size()
		/// * output_reject_levels: false
		#[inline]
		fn detect_multi_scale3(&mut self, image: &impl ToInputArray, objects: &mut core::Vector<core::Rect>, reject_levels: &mut core::Vector<i32>, level_weights: &mut core::Vector<f64>, scale_factor: f64, min_neighbors: i32, flags: i32, min_size: core::Size, max_size: core::Size, output_reject_levels: bool) -> Result<()> {
			input_array_arg!(image);
			return_send!(via ocvrs_return);
			unsafe { sys::cv_CascadeClassifier_detectMultiScale_const__InputArrayR_vectorLRectGR_vectorLintGR_vectorLdoubleGR_double_int_int_Size_Size_bool(self.as_raw_mut_CascadeClassifier(), image.as_raw__InputArray(), objects.as_raw_mut_VectorOfRect(), reject_levels.as_raw_mut_VectorOfi32(), level_weights.as_raw_mut_VectorOff64(), scale_factor, min_neighbors, flags, &min_size, &max_size, output_reject_levels, ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		/// @overload
		/// This function allows you to retrieve the final stage decision certainty of classification.
		/// For this, one needs to set `outputRejectLevels` on true and provide the `rejectLevels` and `levelWeights` parameter.
		/// For each resulting detection, `levelWeights` will then contain the certainty of classification at the final stage.
		/// This value can then be used to separate strong from weaker classifications.
		///
		/// A code sample on how to use it efficiently can be found below:
		/// ```C++
		/// Mat img;
		/// vector<double> weights;
		/// vector<int> levels;
		/// vector<Rect> detections;
		/// CascadeClassifier model("/path/to/your/model.xml");
		/// model.detectMultiScale(img, detections, levels, weights, 1.1, 3, 0, Size(), Size(), true);
		/// cerr << "Detection " << detections[0] << " with weight " << weights[0] << endl;
		/// ```
		///
		///
		/// ## Note
		/// This alternative version of [CascadeClassifierTrait::detect_multi_scale3] function uses the following default values for its arguments:
		/// * scale_factor: 1.1
		/// * min_neighbors: 3
		/// * flags: 0
		/// * min_size: Size()
		/// * max_size: Size()
		/// * output_reject_levels: false
		#[inline]
		fn detect_multi_scale3_def(&mut self, image: &impl ToInputArray, objects: &mut core::Vector<core::Rect>, reject_levels: &mut core::Vector<i32>, level_weights: &mut core::Vector<f64>) -> Result<()> {
			input_array_arg!(image);
			return_send!(via ocvrs_return);
			unsafe { sys::cv_CascadeClassifier_detectMultiScale_const__InputArrayR_vectorLRectGR_vectorLintGR_vectorLdoubleGR(self.as_raw_mut_CascadeClassifier(), image.as_raw__InputArray(), objects.as_raw_mut_VectorOfRect(), reject_levels.as_raw_mut_VectorOfi32(), level_weights.as_raw_mut_VectorOff64(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		#[inline]
		fn get_old_cascade(&mut self) -> Result<*mut c_void> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_CascadeClassifier_getOldCascade(self.as_raw_mut_CascadeClassifier(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		#[inline]
		fn set_mask_generator(&mut self, mask_generator: &core::Ptr<crate::objdetect::BaseCascadeClassifier_MaskGenerator>) -> Result<()> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_CascadeClassifier_setMaskGenerator_const_PtrLMaskGeneratorGR(self.as_raw_mut_CascadeClassifier(), mask_generator.as_raw_PtrOfBaseCascadeClassifier_MaskGenerator(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		#[inline]
		fn get_mask_generator(&mut self) -> Result<core::Ptr<crate::objdetect::BaseCascadeClassifier_MaskGenerator>> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_CascadeClassifier_getMaskGenerator(self.as_raw_mut_CascadeClassifier(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			let ret = unsafe { core::Ptr::<crate::objdetect::BaseCascadeClassifier_MaskGenerator>::opencv_from_extern(ret) };
			Ok(ret)
		}

	}

	impl std::fmt::Debug for CascadeClassifier {
		#[inline]
		fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
			f.debug_struct("CascadeClassifier")
				.finish()
		}
	}

	impl crate::objdetect::CascadeClassifierTraitConst for CascadeClassifier {
		#[inline] fn as_raw_CascadeClassifier(&self) -> *const c_void { self.as_raw() }
	}

	impl crate::objdetect::CascadeClassifierTrait for CascadeClassifier {
		#[inline] fn as_raw_mut_CascadeClassifier(&mut self) -> *mut c_void { self.as_raw_mut() }
	}

	boxed_ref! { CascadeClassifier, crate::objdetect::CascadeClassifierTraitConst, as_raw_CascadeClassifier, crate::objdetect::CascadeClassifierTrait, as_raw_mut_CascadeClassifier }

	pub struct DetectionBasedTracker {
		ptr: *mut c_void,
	}

	opencv_type_boxed! { DetectionBasedTracker }

	impl Drop for DetectionBasedTracker {
		#[inline]
		fn drop(&mut self) {
			unsafe { sys::cv_DetectionBasedTracker_delete(self.as_raw_mut_DetectionBasedTracker()) };
		}
	}

	unsafe impl Send for DetectionBasedTracker {}

	impl DetectionBasedTracker {
		#[inline]
		pub fn new(mut main_detector: core::Ptr<crate::objdetect::DetectionBasedTracker_IDetector>, mut tracking_detector: core::Ptr<crate::objdetect::DetectionBasedTracker_IDetector>, params: &impl crate::objdetect::DetectionBasedTracker_ParametersTraitConst) -> Result<crate::objdetect::DetectionBasedTracker> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_DetectionBasedTracker_DetectionBasedTracker_PtrLIDetectorG_PtrLIDetectorG_const_ParametersR(main_detector.as_raw_mut_PtrOfDetectionBasedTracker_IDetector(), tracking_detector.as_raw_mut_PtrOfDetectionBasedTracker_IDetector(), params.as_raw_DetectionBasedTracker_Parameters(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			let ret = unsafe { crate::objdetect::DetectionBasedTracker::opencv_from_extern(ret) };
			Ok(ret)
		}

	}

	/// Constant methods for [crate::objdetect::DetectionBasedTracker]
	pub trait DetectionBasedTrackerTraitConst {
		fn as_raw_DetectionBasedTracker(&self) -> *const c_void;

		#[inline]
		fn get_parameters(&self) -> Result<crate::objdetect::DetectionBasedTracker_Parameters> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_DetectionBasedTracker_getParameters_const(self.as_raw_DetectionBasedTracker(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			let ret = unsafe { crate::objdetect::DetectionBasedTracker_Parameters::opencv_from_extern(ret) };
			Ok(ret)
		}

		#[inline]
		fn get_objects(&self, result: &mut core::Vector<core::Rect>) -> Result<()> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_DetectionBasedTracker_getObjects_const_vectorLRectGR(self.as_raw_DetectionBasedTracker(), result.as_raw_mut_VectorOfRect(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		#[inline]
		fn get_objects_1(&self, result: &mut core::Vector<crate::objdetect::DetectionBasedTracker_Object>) -> Result<()> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_DetectionBasedTracker_getObjects_const_vectorLObjectGR(self.as_raw_DetectionBasedTracker(), result.as_raw_mut_VectorOfDetectionBasedTracker_Object(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		#[inline]
		fn get_objects_2(&self, result: &mut core::Vector<crate::objdetect::DetectionBasedTracker_ExtObject>) -> Result<()> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_DetectionBasedTracker_getObjects_const_vectorLExtObjectGR(self.as_raw_DetectionBasedTracker(), result.as_raw_mut_VectorOfDetectionBasedTracker_ExtObject(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

	}

	/// Mutable methods for [crate::objdetect::DetectionBasedTracker]
	pub trait DetectionBasedTrackerTrait: crate::objdetect::DetectionBasedTrackerTraitConst {
		fn as_raw_mut_DetectionBasedTracker(&mut self) -> *mut c_void;

		#[inline]
		fn run(&mut self) -> Result<bool> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_DetectionBasedTracker_run(self.as_raw_mut_DetectionBasedTracker(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		#[inline]
		fn stop(&mut self) -> Result<()> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_DetectionBasedTracker_stop(self.as_raw_mut_DetectionBasedTracker(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		#[inline]
		fn reset_tracking(&mut self) -> Result<()> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_DetectionBasedTracker_resetTracking(self.as_raw_mut_DetectionBasedTracker(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		#[inline]
		fn process(&mut self, image_gray: &impl core::MatTraitConst) -> Result<()> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_DetectionBasedTracker_process_const_MatR(self.as_raw_mut_DetectionBasedTracker(), image_gray.as_raw_Mat(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		#[inline]
		fn set_parameters(&mut self, params: &impl crate::objdetect::DetectionBasedTracker_ParametersTraitConst) -> Result<bool> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_DetectionBasedTracker_setParameters_const_ParametersR(self.as_raw_mut_DetectionBasedTracker(), params.as_raw_DetectionBasedTracker_Parameters(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		#[inline]
		fn add_object(&mut self, location: core::Rect) -> Result<i32> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_DetectionBasedTracker_addObject_const_RectR(self.as_raw_mut_DetectionBasedTracker(), &location, ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

	}

	impl std::fmt::Debug for DetectionBasedTracker {
		#[inline]
		fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
			f.debug_struct("DetectionBasedTracker")
				.finish()
		}
	}

	impl crate::objdetect::DetectionBasedTrackerTraitConst for DetectionBasedTracker {
		#[inline] fn as_raw_DetectionBasedTracker(&self) -> *const c_void { self.as_raw() }
	}

	impl crate::objdetect::DetectionBasedTrackerTrait for DetectionBasedTracker {
		#[inline] fn as_raw_mut_DetectionBasedTracker(&mut self) -> *mut c_void { self.as_raw_mut() }
	}

	boxed_ref! { DetectionBasedTracker, crate::objdetect::DetectionBasedTrackerTraitConst, as_raw_DetectionBasedTracker, crate::objdetect::DetectionBasedTrackerTrait, as_raw_mut_DetectionBasedTracker }

	pub struct DetectionBasedTracker_ExtObject {
		ptr: *mut c_void,
	}

	opencv_type_boxed! { DetectionBasedTracker_ExtObject }

	impl Drop for DetectionBasedTracker_ExtObject {
		#[inline]
		fn drop(&mut self) {
			unsafe { sys::cv_DetectionBasedTracker_ExtObject_delete(self.as_raw_mut_DetectionBasedTracker_ExtObject()) };
		}
	}

	unsafe impl Send for DetectionBasedTracker_ExtObject {}

	impl DetectionBasedTracker_ExtObject {
		#[inline]
		pub fn new(_id: i32, _location: core::Rect, _status: crate::objdetect::DetectionBasedTracker_ObjectStatus) -> Result<crate::objdetect::DetectionBasedTracker_ExtObject> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_DetectionBasedTracker_ExtObject_ExtObject_int_Rect_ObjectStatus(_id, &_location, _status, ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			let ret = unsafe { crate::objdetect::DetectionBasedTracker_ExtObject::opencv_from_extern(ret) };
			Ok(ret)
		}

	}

	/// Constant methods for [crate::objdetect::DetectionBasedTracker_ExtObject]
	pub trait DetectionBasedTracker_ExtObjectTraitConst {
		fn as_raw_DetectionBasedTracker_ExtObject(&self) -> *const c_void;

		#[inline]
		fn id(&self) -> i32 {
			let ret = unsafe { sys::cv_DetectionBasedTracker_ExtObject_propId_const(self.as_raw_DetectionBasedTracker_ExtObject()) };
			ret
		}

		#[inline]
		fn location(&self) -> core::Rect {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_DetectionBasedTracker_ExtObject_propLocation_const(self.as_raw_DetectionBasedTracker_ExtObject(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			ret
		}

		#[inline]
		fn status(&self) -> crate::objdetect::DetectionBasedTracker_ObjectStatus {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_DetectionBasedTracker_ExtObject_propStatus_const(self.as_raw_DetectionBasedTracker_ExtObject(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			ret
		}

	}

	/// Mutable methods for [crate::objdetect::DetectionBasedTracker_ExtObject]
	pub trait DetectionBasedTracker_ExtObjectTrait: crate::objdetect::DetectionBasedTracker_ExtObjectTraitConst {
		fn as_raw_mut_DetectionBasedTracker_ExtObject(&mut self) -> *mut c_void;

		#[inline]
		fn set_id(&mut self, val: i32) {
			let ret = unsafe { sys::cv_DetectionBasedTracker_ExtObject_propId_const_int(self.as_raw_mut_DetectionBasedTracker_ExtObject(), val) };
			ret
		}

		#[inline]
		fn set_location(&mut self, val: core::Rect) {
			let ret = unsafe { sys::cv_DetectionBasedTracker_ExtObject_propLocation_const_Rect(self.as_raw_mut_DetectionBasedTracker_ExtObject(), &val) };
			ret
		}

		#[inline]
		fn set_status(&mut self, val: crate::objdetect::DetectionBasedTracker_ObjectStatus) {
			let ret = unsafe { sys::cv_DetectionBasedTracker_ExtObject_propStatus_const_ObjectStatus(self.as_raw_mut_DetectionBasedTracker_ExtObject(), val) };
			ret
		}

	}

	impl Clone for DetectionBasedTracker_ExtObject {
		#[inline]
		fn clone(&self) -> Self {
			unsafe { Self::from_raw(sys::cv_DetectionBasedTracker_ExtObject_implicitClone_const(self.as_raw_DetectionBasedTracker_ExtObject())) }
		}
	}

	impl std::fmt::Debug for DetectionBasedTracker_ExtObject {
		#[inline]
		fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
			f.debug_struct("DetectionBasedTracker_ExtObject")
				.field("id", &crate::objdetect::DetectionBasedTracker_ExtObjectTraitConst::id(self))
				.field("location", &crate::objdetect::DetectionBasedTracker_ExtObjectTraitConst::location(self))
				.field("status", &crate::objdetect::DetectionBasedTracker_ExtObjectTraitConst::status(self))
				.finish()
		}
	}

	impl crate::objdetect::DetectionBasedTracker_ExtObjectTraitConst for DetectionBasedTracker_ExtObject {
		#[inline] fn as_raw_DetectionBasedTracker_ExtObject(&self) -> *const c_void { self.as_raw() }
	}

	impl crate::objdetect::DetectionBasedTracker_ExtObjectTrait for DetectionBasedTracker_ExtObject {
		#[inline] fn as_raw_mut_DetectionBasedTracker_ExtObject(&mut self) -> *mut c_void { self.as_raw_mut() }
	}

	boxed_ref! { DetectionBasedTracker_ExtObject, crate::objdetect::DetectionBasedTracker_ExtObjectTraitConst, as_raw_DetectionBasedTracker_ExtObject, crate::objdetect::DetectionBasedTracker_ExtObjectTrait, as_raw_mut_DetectionBasedTracker_ExtObject }

	pub struct DetectionBasedTracker_IDetector {
		ptr: *mut c_void,
	}

	opencv_type_boxed! { DetectionBasedTracker_IDetector }

	impl Drop for DetectionBasedTracker_IDetector {
		#[inline]
		fn drop(&mut self) {
			unsafe { sys::cv_DetectionBasedTracker_IDetector_delete(self.as_raw_mut_DetectionBasedTracker_IDetector()) };
		}
	}

	unsafe impl Send for DetectionBasedTracker_IDetector {}

	/// Constant methods for [crate::objdetect::DetectionBasedTracker_IDetector]
	pub trait DetectionBasedTracker_IDetectorTraitConst {
		fn as_raw_DetectionBasedTracker_IDetector(&self) -> *const c_void;

		#[inline]
		fn get_min_object_size(&self) -> Result<core::Size> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_DetectionBasedTracker_IDetector_getMinObjectSize_const(self.as_raw_DetectionBasedTracker_IDetector(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		#[inline]
		fn get_max_object_size(&self) -> Result<core::Size> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_DetectionBasedTracker_IDetector_getMaxObjectSize_const(self.as_raw_DetectionBasedTracker_IDetector(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

	}

	/// Mutable methods for [crate::objdetect::DetectionBasedTracker_IDetector]
	pub trait DetectionBasedTracker_IDetectorTrait: crate::objdetect::DetectionBasedTracker_IDetectorTraitConst {
		fn as_raw_mut_DetectionBasedTracker_IDetector(&mut self) -> *mut c_void;

		#[inline]
		fn detect(&mut self, image: &impl core::MatTraitConst, objects: &mut core::Vector<core::Rect>) -> Result<()> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_DetectionBasedTracker_IDetector_detect_const_MatR_vectorLRectGR(self.as_raw_mut_DetectionBasedTracker_IDetector(), image.as_raw_Mat(), objects.as_raw_mut_VectorOfRect(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		#[inline]
		fn set_min_object_size(&mut self, min: core::Size) -> Result<()> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_DetectionBasedTracker_IDetector_setMinObjectSize_const_SizeR(self.as_raw_mut_DetectionBasedTracker_IDetector(), &min, ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		#[inline]
		fn set_max_object_size(&mut self, max: core::Size) -> Result<()> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_DetectionBasedTracker_IDetector_setMaxObjectSize_const_SizeR(self.as_raw_mut_DetectionBasedTracker_IDetector(), &max, ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		#[inline]
		fn get_scale_factor(&mut self) -> Result<f32> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_DetectionBasedTracker_IDetector_getScaleFactor(self.as_raw_mut_DetectionBasedTracker_IDetector(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		#[inline]
		fn set_scale_factor(&mut self, value: f32) -> Result<()> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_DetectionBasedTracker_IDetector_setScaleFactor_float(self.as_raw_mut_DetectionBasedTracker_IDetector(), value, ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		#[inline]
		fn get_min_neighbours(&mut self) -> Result<i32> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_DetectionBasedTracker_IDetector_getMinNeighbours(self.as_raw_mut_DetectionBasedTracker_IDetector(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		#[inline]
		fn set_min_neighbours(&mut self, value: i32) -> Result<()> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_DetectionBasedTracker_IDetector_setMinNeighbours_int(self.as_raw_mut_DetectionBasedTracker_IDetector(), value, ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

	}

	impl std::fmt::Debug for DetectionBasedTracker_IDetector {
		#[inline]
		fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
			f.debug_struct("DetectionBasedTracker_IDetector")
				.finish()
		}
	}

	impl crate::objdetect::DetectionBasedTracker_IDetectorTraitConst for DetectionBasedTracker_IDetector {
		#[inline] fn as_raw_DetectionBasedTracker_IDetector(&self) -> *const c_void { self.as_raw() }
	}

	impl crate::objdetect::DetectionBasedTracker_IDetectorTrait for DetectionBasedTracker_IDetector {
		#[inline] fn as_raw_mut_DetectionBasedTracker_IDetector(&mut self) -> *mut c_void { self.as_raw_mut() }
	}

	boxed_ref! { DetectionBasedTracker_IDetector, crate::objdetect::DetectionBasedTracker_IDetectorTraitConst, as_raw_DetectionBasedTracker_IDetector, crate::objdetect::DetectionBasedTracker_IDetectorTrait, as_raw_mut_DetectionBasedTracker_IDetector }

	pub struct DetectionBasedTracker_Parameters {
		ptr: *mut c_void,
	}

	opencv_type_boxed! { DetectionBasedTracker_Parameters }

	impl Drop for DetectionBasedTracker_Parameters {
		#[inline]
		fn drop(&mut self) {
			unsafe { sys::cv_DetectionBasedTracker_Parameters_delete(self.as_raw_mut_DetectionBasedTracker_Parameters()) };
		}
	}

	unsafe impl Send for DetectionBasedTracker_Parameters {}

	impl DetectionBasedTracker_Parameters {
		#[inline]
		pub fn default() -> Result<crate::objdetect::DetectionBasedTracker_Parameters> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_DetectionBasedTracker_Parameters_Parameters(ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			let ret = unsafe { crate::objdetect::DetectionBasedTracker_Parameters::opencv_from_extern(ret) };
			Ok(ret)
		}

	}

	/// Constant methods for [crate::objdetect::DetectionBasedTracker_Parameters]
	pub trait DetectionBasedTracker_ParametersTraitConst {
		fn as_raw_DetectionBasedTracker_Parameters(&self) -> *const c_void;

		#[inline]
		fn max_track_lifetime(&self) -> i32 {
			let ret = unsafe { sys::cv_DetectionBasedTracker_Parameters_propMaxTrackLifetime_const(self.as_raw_DetectionBasedTracker_Parameters()) };
			ret
		}

		#[inline]
		fn min_detection_period(&self) -> i32 {
			let ret = unsafe { sys::cv_DetectionBasedTracker_Parameters_propMinDetectionPeriod_const(self.as_raw_DetectionBasedTracker_Parameters()) };
			ret
		}

	}

	/// Mutable methods for [crate::objdetect::DetectionBasedTracker_Parameters]
	pub trait DetectionBasedTracker_ParametersTrait: crate::objdetect::DetectionBasedTracker_ParametersTraitConst {
		fn as_raw_mut_DetectionBasedTracker_Parameters(&mut self) -> *mut c_void;

		#[inline]
		fn set_max_track_lifetime(&mut self, val: i32) {
			let ret = unsafe { sys::cv_DetectionBasedTracker_Parameters_propMaxTrackLifetime_const_int(self.as_raw_mut_DetectionBasedTracker_Parameters(), val) };
			ret
		}

		#[inline]
		fn set_min_detection_period(&mut self, val: i32) {
			let ret = unsafe { sys::cv_DetectionBasedTracker_Parameters_propMinDetectionPeriod_const_int(self.as_raw_mut_DetectionBasedTracker_Parameters(), val) };
			ret
		}

	}

	impl std::fmt::Debug for DetectionBasedTracker_Parameters {
		#[inline]
		fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
			f.debug_struct("DetectionBasedTracker_Parameters")
				.field("max_track_lifetime", &crate::objdetect::DetectionBasedTracker_ParametersTraitConst::max_track_lifetime(self))
				.field("min_detection_period", &crate::objdetect::DetectionBasedTracker_ParametersTraitConst::min_detection_period(self))
				.finish()
		}
	}

	impl crate::objdetect::DetectionBasedTracker_ParametersTraitConst for DetectionBasedTracker_Parameters {
		#[inline] fn as_raw_DetectionBasedTracker_Parameters(&self) -> *const c_void { self.as_raw() }
	}

	impl crate::objdetect::DetectionBasedTracker_ParametersTrait for DetectionBasedTracker_Parameters {
		#[inline] fn as_raw_mut_DetectionBasedTracker_Parameters(&mut self) -> *mut c_void { self.as_raw_mut() }
	}

	boxed_ref! { DetectionBasedTracker_Parameters, crate::objdetect::DetectionBasedTracker_ParametersTraitConst, as_raw_DetectionBasedTracker_Parameters, crate::objdetect::DetectionBasedTracker_ParametersTrait, as_raw_mut_DetectionBasedTracker_Parameters }

	/// struct for detection region of interest (ROI)
	pub struct DetectionROI {
		ptr: *mut c_void,
	}

	opencv_type_boxed! { DetectionROI }

	impl Drop for DetectionROI {
		#[inline]
		fn drop(&mut self) {
			unsafe { sys::cv_DetectionROI_delete(self.as_raw_mut_DetectionROI()) };
		}
	}

	unsafe impl Send for DetectionROI {}

	impl DetectionROI {
		/// Creates a default instance of the class by calling the default constructor
		#[inline]
		pub fn default() -> crate::objdetect::DetectionROI {
			let ret = unsafe { sys::cv_DetectionROI_defaultNew_const() };
			let ret = unsafe { crate::objdetect::DetectionROI::opencv_from_extern(ret) };
			ret
		}

	}

	/// Constant methods for [crate::objdetect::DetectionROI]
	pub trait DetectionROITraitConst {
		fn as_raw_DetectionROI(&self) -> *const c_void;

		/// scale(size) of the bounding box
		#[inline]
		fn scale(&self) -> f64 {
			let ret = unsafe { sys::cv_DetectionROI_propScale_const(self.as_raw_DetectionROI()) };
			ret
		}

		/// set of requested locations to be evaluated
		#[inline]
		fn locations(&self) -> core::Vector<core::Point> {
			let ret = unsafe { sys::cv_DetectionROI_propLocations_const(self.as_raw_DetectionROI()) };
			let ret = unsafe { core::Vector::<core::Point>::opencv_from_extern(ret) };
			ret
		}

		/// vector that will contain confidence values for each location
		#[inline]
		fn confidences(&self) -> core::Vector<f64> {
			let ret = unsafe { sys::cv_DetectionROI_propConfidences_const(self.as_raw_DetectionROI()) };
			let ret = unsafe { core::Vector::<f64>::opencv_from_extern(ret) };
			ret
		}

	}

	/// Mutable methods for [crate::objdetect::DetectionROI]
	pub trait DetectionROITrait: crate::objdetect::DetectionROITraitConst {
		fn as_raw_mut_DetectionROI(&mut self) -> *mut c_void;

		/// scale(size) of the bounding box
		#[inline]
		fn set_scale(&mut self, val: f64) {
			let ret = unsafe { sys::cv_DetectionROI_propScale_const_double(self.as_raw_mut_DetectionROI(), val) };
			ret
		}

		/// set of requested locations to be evaluated
		#[inline]
		fn set_locations(&mut self, val: core::Vector<core::Point>) {
			let ret = unsafe { sys::cv_DetectionROI_propLocations_const_vectorLPointG(self.as_raw_mut_DetectionROI(), val.as_raw_VectorOfPoint()) };
			ret
		}

		/// vector that will contain confidence values for each location
		#[inline]
		fn set_confidences(&mut self, val: core::Vector<f64>) {
			let ret = unsafe { sys::cv_DetectionROI_propConfidences_const_vectorLdoubleG(self.as_raw_mut_DetectionROI(), val.as_raw_VectorOff64()) };
			ret
		}

	}

	impl Default for DetectionROI {
		#[inline]
		/// Forwards to infallible Self::default()
		fn default() -> Self {
			Self::default()
		}
	}

	impl std::fmt::Debug for DetectionROI {
		#[inline]
		fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
			f.debug_struct("DetectionROI")
				.field("scale", &crate::objdetect::DetectionROITraitConst::scale(self))
				.field("locations", &crate::objdetect::DetectionROITraitConst::locations(self))
				.field("confidences", &crate::objdetect::DetectionROITraitConst::confidences(self))
				.finish()
		}
	}

	impl crate::objdetect::DetectionROITraitConst for DetectionROI {
		#[inline] fn as_raw_DetectionROI(&self) -> *const c_void { self.as_raw() }
	}

	impl crate::objdetect::DetectionROITrait for DetectionROI {
		#[inline] fn as_raw_mut_DetectionROI(&mut self) -> *mut c_void { self.as_raw_mut() }
	}

	boxed_ref! { DetectionROI, crate::objdetect::DetectionROITraitConst, as_raw_DetectionROI, crate::objdetect::DetectionROITrait, as_raw_mut_DetectionROI }

	/// DNN-based face detector
	///
	/// model download link: <https://github.com/opencv/opencv_zoo/tree/master/models/face_detection_yunet>
	pub struct FaceDetectorYN {
		ptr: *mut c_void,
	}

	opencv_type_boxed! { FaceDetectorYN }

	impl Drop for FaceDetectorYN {
		#[inline]
		fn drop(&mut self) {
			unsafe { sys::cv_FaceDetectorYN_delete(self.as_raw_mut_FaceDetectorYN()) };
		}
	}

	unsafe impl Send for FaceDetectorYN {}

	impl FaceDetectorYN {
		/// Creates an instance of this class with given parameters
		///
		/// ## Parameters
		/// * model: the path to the requested model
		/// * config: the path to the config file for compability, which is not requested for ONNX models
		/// * input_size: the size of the input image
		/// * score_threshold: the threshold to filter out bounding boxes of score smaller than the given value
		/// * nms_threshold: the threshold to suppress bounding boxes of IoU bigger than the given value
		/// * top_k: keep top K bboxes before NMS
		/// * backend_id: the id of backend
		/// * target_id: the id of target device
		///
		/// ## C++ default parameters
		/// * score_threshold: 0.9f
		/// * nms_threshold: 0.3f
		/// * top_k: 5000
		/// * backend_id: 0
		/// * target_id: 0
		#[inline]
		pub fn create(model: &str, config: &str, input_size: core::Size, score_threshold: f32, nms_threshold: f32, top_k: i32, backend_id: i32, target_id: i32) -> Result<core::Ptr<crate::objdetect::FaceDetectorYN>> {
			extern_container_arg!(model);
			extern_container_arg!(config);
			return_send!(via ocvrs_return);
			unsafe { sys::cv_FaceDetectorYN_create_const_StringR_const_StringR_const_SizeR_float_float_int_int_int(model.opencv_as_extern(), config.opencv_as_extern(), &input_size, score_threshold, nms_threshold, top_k, backend_id, target_id, ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			let ret = unsafe { core::Ptr::<crate::objdetect::FaceDetectorYN>::opencv_from_extern(ret) };
			Ok(ret)
		}

		/// Creates an instance of this class with given parameters
		///
		/// ## Parameters
		/// * model: the path to the requested model
		/// * config: the path to the config file for compability, which is not requested for ONNX models
		/// * input_size: the size of the input image
		/// * score_threshold: the threshold to filter out bounding boxes of score smaller than the given value
		/// * nms_threshold: the threshold to suppress bounding boxes of IoU bigger than the given value
		/// * top_k: keep top K bboxes before NMS
		/// * backend_id: the id of backend
		/// * target_id: the id of target device
		///
		/// ## Note
		/// This alternative version of [FaceDetectorYN::create] function uses the following default values for its arguments:
		/// * score_threshold: 0.9f
		/// * nms_threshold: 0.3f
		/// * top_k: 5000
		/// * backend_id: 0
		/// * target_id: 0
		#[inline]
		pub fn create_def(model: &str, config: &str, input_size: core::Size) -> Result<core::Ptr<crate::objdetect::FaceDetectorYN>> {
			extern_container_arg!(model);
			extern_container_arg!(config);
			return_send!(via ocvrs_return);
			unsafe { sys::cv_FaceDetectorYN_create_const_StringR_const_StringR_const_SizeR(model.opencv_as_extern(), config.opencv_as_extern(), &input_size, ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			let ret = unsafe { core::Ptr::<crate::objdetect::FaceDetectorYN>::opencv_from_extern(ret) };
			Ok(ret)
		}

	}

	/// Constant methods for [crate::objdetect::FaceDetectorYN]
	pub trait FaceDetectorYNTraitConst {
		fn as_raw_FaceDetectorYN(&self) -> *const c_void;

	}

	/// Mutable methods for [crate::objdetect::FaceDetectorYN]
	pub trait FaceDetectorYNTrait: crate::objdetect::FaceDetectorYNTraitConst {
		fn as_raw_mut_FaceDetectorYN(&mut self) -> *mut c_void;

		/// Set the size for the network input, which overwrites the input size of creating model. Call this method when the size of input image does not match the input size when creating model
		///
		/// ## Parameters
		/// * input_size: the size of the input image
		#[inline]
		fn set_input_size(&mut self, input_size: core::Size) -> Result<()> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_FaceDetectorYN_setInputSize_const_SizeR(self.as_raw_mut_FaceDetectorYN(), &input_size, ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		#[inline]
		fn get_input_size(&mut self) -> Result<core::Size> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_FaceDetectorYN_getInputSize(self.as_raw_mut_FaceDetectorYN(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		/// Set the score threshold to filter out bounding boxes of score less than the given value
		///
		/// ## Parameters
		/// * score_threshold: threshold for filtering out bounding boxes
		#[inline]
		fn set_score_threshold(&mut self, score_threshold: f32) -> Result<()> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_FaceDetectorYN_setScoreThreshold_float(self.as_raw_mut_FaceDetectorYN(), score_threshold, ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		#[inline]
		fn get_score_threshold(&mut self) -> Result<f32> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_FaceDetectorYN_getScoreThreshold(self.as_raw_mut_FaceDetectorYN(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		/// Set the Non-maximum-suppression threshold to suppress bounding boxes that have IoU greater than the given value
		///
		/// ## Parameters
		/// * nms_threshold: threshold for NMS operation
		#[inline]
		fn set_nms_threshold(&mut self, nms_threshold: f32) -> Result<()> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_FaceDetectorYN_setNMSThreshold_float(self.as_raw_mut_FaceDetectorYN(), nms_threshold, ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		#[inline]
		fn get_nms_threshold(&mut self) -> Result<f32> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_FaceDetectorYN_getNMSThreshold(self.as_raw_mut_FaceDetectorYN(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		/// Set the number of bounding boxes preserved before NMS
		///
		/// ## Parameters
		/// * top_k: the number of bounding boxes to preserve from top rank based on score
		#[inline]
		fn set_top_k(&mut self, top_k: i32) -> Result<()> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_FaceDetectorYN_setTopK_int(self.as_raw_mut_FaceDetectorYN(), top_k, ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		#[inline]
		fn get_top_k(&mut self) -> Result<i32> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_FaceDetectorYN_getTopK(self.as_raw_mut_FaceDetectorYN(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		/// A simple interface to detect face from given image
		///
		/// ## Parameters
		/// * image: an image to detect
		/// * faces: detection results stored in a cv::Mat
		#[inline]
		fn detect(&mut self, image: &impl ToInputArray, faces: &mut impl ToOutputArray) -> Result<i32> {
			input_array_arg!(image);
			output_array_arg!(faces);
			return_send!(via ocvrs_return);
			unsafe { sys::cv_FaceDetectorYN_detect_const__InputArrayR_const__OutputArrayR(self.as_raw_mut_FaceDetectorYN(), image.as_raw__InputArray(), faces.as_raw__OutputArray(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

	}

	impl std::fmt::Debug for FaceDetectorYN {
		#[inline]
		fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
			f.debug_struct("FaceDetectorYN")
				.finish()
		}
	}

	impl crate::objdetect::FaceDetectorYNTraitConst for FaceDetectorYN {
		#[inline] fn as_raw_FaceDetectorYN(&self) -> *const c_void { self.as_raw() }
	}

	impl crate::objdetect::FaceDetectorYNTrait for FaceDetectorYN {
		#[inline] fn as_raw_mut_FaceDetectorYN(&mut self) -> *mut c_void { self.as_raw_mut() }
	}

	boxed_ref! { FaceDetectorYN, crate::objdetect::FaceDetectorYNTraitConst, as_raw_FaceDetectorYN, crate::objdetect::FaceDetectorYNTrait, as_raw_mut_FaceDetectorYN }

	/// DNN-based face recognizer
	///
	/// model download link: <https://github.com/opencv/opencv_zoo/tree/master/models/face_recognition_sface>
	pub struct FaceRecognizerSF {
		ptr: *mut c_void,
	}

	opencv_type_boxed! { FaceRecognizerSF }

	impl Drop for FaceRecognizerSF {
		#[inline]
		fn drop(&mut self) {
			unsafe { sys::cv_FaceRecognizerSF_delete(self.as_raw_mut_FaceRecognizerSF()) };
		}
	}

	unsafe impl Send for FaceRecognizerSF {}

	impl FaceRecognizerSF {
		/// Creates an instance of this class with given parameters
		/// ## Parameters
		/// * model: the path of the onnx model used for face recognition
		/// * config: the path to the config file for compability, which is not requested for ONNX models
		/// * backend_id: the id of backend
		/// * target_id: the id of target device
		///
		/// ## C++ default parameters
		/// * backend_id: 0
		/// * target_id: 0
		#[inline]
		pub fn create(model: &str, config: &str, backend_id: i32, target_id: i32) -> Result<core::Ptr<crate::objdetect::FaceRecognizerSF>> {
			extern_container_arg!(model);
			extern_container_arg!(config);
			return_send!(via ocvrs_return);
			unsafe { sys::cv_FaceRecognizerSF_create_const_StringR_const_StringR_int_int(model.opencv_as_extern(), config.opencv_as_extern(), backend_id, target_id, ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			let ret = unsafe { core::Ptr::<crate::objdetect::FaceRecognizerSF>::opencv_from_extern(ret) };
			Ok(ret)
		}

		/// Creates an instance of this class with given parameters
		/// ## Parameters
		/// * model: the path of the onnx model used for face recognition
		/// * config: the path to the config file for compability, which is not requested for ONNX models
		/// * backend_id: the id of backend
		/// * target_id: the id of target device
		///
		/// ## Note
		/// This alternative version of [FaceRecognizerSF::create] function uses the following default values for its arguments:
		/// * backend_id: 0
		/// * target_id: 0
		#[inline]
		pub fn create_def(model: &str, config: &str) -> Result<core::Ptr<crate::objdetect::FaceRecognizerSF>> {
			extern_container_arg!(model);
			extern_container_arg!(config);
			return_send!(via ocvrs_return);
			unsafe { sys::cv_FaceRecognizerSF_create_const_StringR_const_StringR(model.opencv_as_extern(), config.opencv_as_extern(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			let ret = unsafe { core::Ptr::<crate::objdetect::FaceRecognizerSF>::opencv_from_extern(ret) };
			Ok(ret)
		}

	}

	/// Constant methods for [crate::objdetect::FaceRecognizerSF]
	pub trait FaceRecognizerSFTraitConst {
		fn as_raw_FaceRecognizerSF(&self) -> *const c_void;

		/// Aligning image to put face on the standard position
		/// ## Parameters
		/// * src_img: input image
		/// * face_box: the detection result used for indicate face in input image
		/// * aligned_img: output aligned image
		#[inline]
		fn align_crop(&self, src_img: &impl ToInputArray, face_box: &impl ToInputArray, aligned_img: &mut impl ToOutputArray) -> Result<()> {
			input_array_arg!(src_img);
			input_array_arg!(face_box);
			output_array_arg!(aligned_img);
			return_send!(via ocvrs_return);
			unsafe { sys::cv_FaceRecognizerSF_alignCrop_const_const__InputArrayR_const__InputArrayR_const__OutputArrayR(self.as_raw_FaceRecognizerSF(), src_img.as_raw__InputArray(), face_box.as_raw__InputArray(), aligned_img.as_raw__OutputArray(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		/// Calculating the distance between two face features
		/// ## Parameters
		/// * face_feature1: the first input feature
		/// * face_feature2: the second input feature of the same size and the same type as face_feature1
		/// * dis_type: defining the similarity with optional values "FR_OSINE" or "FR_NORM_L2"
		///
		/// ## C++ default parameters
		/// * dis_type: FaceRecognizerSF::FR_COSINE
		#[inline]
		fn match_(&self, face_feature1: &impl ToInputArray, face_feature2: &impl ToInputArray, dis_type: i32) -> Result<f64> {
			input_array_arg!(face_feature1);
			input_array_arg!(face_feature2);
			return_send!(via ocvrs_return);
			unsafe { sys::cv_FaceRecognizerSF_match_const_const__InputArrayR_const__InputArrayR_int(self.as_raw_FaceRecognizerSF(), face_feature1.as_raw__InputArray(), face_feature2.as_raw__InputArray(), dis_type, ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		/// Calculating the distance between two face features
		/// ## Parameters
		/// * face_feature1: the first input feature
		/// * face_feature2: the second input feature of the same size and the same type as face_feature1
		/// * dis_type: defining the similarity with optional values "FR_OSINE" or "FR_NORM_L2"
		///
		/// ## Note
		/// This alternative version of [FaceRecognizerSFTraitConst::match_] function uses the following default values for its arguments:
		/// * dis_type: FaceRecognizerSF::FR_COSINE
		#[inline]
		fn match__def(&self, face_feature1: &impl ToInputArray, face_feature2: &impl ToInputArray) -> Result<f64> {
			input_array_arg!(face_feature1);
			input_array_arg!(face_feature2);
			return_send!(via ocvrs_return);
			unsafe { sys::cv_FaceRecognizerSF_match_const_const__InputArrayR_const__InputArrayR(self.as_raw_FaceRecognizerSF(), face_feature1.as_raw__InputArray(), face_feature2.as_raw__InputArray(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

	}

	/// Mutable methods for [crate::objdetect::FaceRecognizerSF]
	pub trait FaceRecognizerSFTrait: crate::objdetect::FaceRecognizerSFTraitConst {
		fn as_raw_mut_FaceRecognizerSF(&mut self) -> *mut c_void;

		/// Extracting face feature from aligned image
		/// ## Parameters
		/// * aligned_img: input aligned image
		/// * face_feature: output face feature
		#[inline]
		fn feature(&mut self, aligned_img: &impl ToInputArray, face_feature: &mut impl ToOutputArray) -> Result<()> {
			input_array_arg!(aligned_img);
			output_array_arg!(face_feature);
			return_send!(via ocvrs_return);
			unsafe { sys::cv_FaceRecognizerSF_feature_const__InputArrayR_const__OutputArrayR(self.as_raw_mut_FaceRecognizerSF(), aligned_img.as_raw__InputArray(), face_feature.as_raw__OutputArray(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

	}

	impl std::fmt::Debug for FaceRecognizerSF {
		#[inline]
		fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
			f.debug_struct("FaceRecognizerSF")
				.finish()
		}
	}

	impl crate::objdetect::FaceRecognizerSFTraitConst for FaceRecognizerSF {
		#[inline] fn as_raw_FaceRecognizerSF(&self) -> *const c_void { self.as_raw() }
	}

	impl crate::objdetect::FaceRecognizerSFTrait for FaceRecognizerSF {
		#[inline] fn as_raw_mut_FaceRecognizerSF(&mut self) -> *mut c_void { self.as_raw_mut() }
	}

	boxed_ref! { FaceRecognizerSF, crate::objdetect::FaceRecognizerSFTraitConst, as_raw_FaceRecognizerSF, crate::objdetect::FaceRecognizerSFTrait, as_raw_mut_FaceRecognizerSF }

	/// Implementation of HOG (Histogram of Oriented Gradients) descriptor and object detector.
	///
	/// the HOG descriptor algorithm introduced by Navneet Dalal and Bill Triggs [Dalal2005](https://docs.opencv.org/4.6.0/d0/de3/citelist.html#CITEREF_Dalal2005) .
	///
	/// useful links:
	///
	/// <https://hal.inria.fr/inria-00548512/document/>
	///
	/// <https://en.wikipedia.org/wiki/Histogram_of_oriented_gradients>
	///
	/// <https://software.intel.com/en-us/ipp-dev-reference-histogram-of-oriented-gradients-hog-descriptor>
	///
	/// <http://www.learnopencv.com/histogram-of-oriented-gradients>
	///
	/// <http://www.learnopencv.com/handwritten-digits-classification-an-opencv-c-python-tutorial>
	pub struct HOGDescriptor {
		ptr: *mut c_void,
	}

	opencv_type_boxed! { HOGDescriptor }

	impl Drop for HOGDescriptor {
		#[inline]
		fn drop(&mut self) {
			unsafe { sys::cv_HOGDescriptor_delete(self.as_raw_mut_HOGDescriptor()) };
		}
	}

	unsafe impl Send for HOGDescriptor {}

	impl HOGDescriptor {
		/// Creates the HOG descriptor and detector with default parameters.
		///
		/// aqual to HOGDescriptor(Size(64,128), Size(16,16), Size(8,8), Size(8,8), 9 )
		#[inline]
		pub fn default() -> Result<crate::objdetect::HOGDescriptor> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_HOGDescriptor_HOGDescriptor(ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			let ret = unsafe { crate::objdetect::HOGDescriptor::opencv_from_extern(ret) };
			Ok(ret)
		}

		/// Creates the HOG descriptor and detector with default parameters.
		///
		/// aqual to HOGDescriptor(Size(64,128), Size(16,16), Size(8,8), Size(8,8), 9 )
		///
		/// ## Overloaded parameters
		///
		/// ## Parameters
		/// * _winSize: sets winSize with given value.
		/// * _blockSize: sets blockSize with given value.
		/// * _blockStride: sets blockStride with given value.
		/// * _cellSize: sets cellSize with given value.
		/// * _nbins: sets nbins with given value.
		/// * _derivAperture: sets derivAperture with given value.
		/// * _winSigma: sets winSigma with given value.
		/// * _histogramNormType: sets histogramNormType with given value.
		/// * _L2HysThreshold: sets L2HysThreshold with given value.
		/// * _gammaCorrection: sets gammaCorrection with given value.
		/// * _nlevels: sets nlevels with given value.
		/// * _signedGradient: sets signedGradient with given value.
		///
		/// ## C++ default parameters
		/// * _deriv_aperture: 1
		/// * _win_sigma: -1
		/// * _histogram_norm_type: HOGDescriptor::L2Hys
		/// * _l2_hys_threshold: 0.2
		/// * _gamma_correction: false
		/// * _nlevels: HOGDescriptor::DEFAULT_NLEVELS
		/// * _signed_gradient: false
		#[inline]
		pub fn new(_win_size: core::Size, _block_size: core::Size, _block_stride: core::Size, _cell_size: core::Size, _nbins: i32, _deriv_aperture: i32, _win_sigma: f64, _histogram_norm_type: crate::objdetect::HOGDescriptor_HistogramNormType, _l2_hys_threshold: f64, _gamma_correction: bool, _nlevels: i32, _signed_gradient: bool) -> Result<crate::objdetect::HOGDescriptor> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_HOGDescriptor_HOGDescriptor_Size_Size_Size_Size_int_int_double_HistogramNormType_double_bool_int_bool(&_win_size, &_block_size, &_block_stride, &_cell_size, _nbins, _deriv_aperture, _win_sigma, _histogram_norm_type, _l2_hys_threshold, _gamma_correction, _nlevels, _signed_gradient, ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			let ret = unsafe { crate::objdetect::HOGDescriptor::opencv_from_extern(ret) };
			Ok(ret)
		}

		/// @overload
		/// ## Parameters
		/// * _winSize: sets winSize with given value.
		/// * _blockSize: sets blockSize with given value.
		/// * _blockStride: sets blockStride with given value.
		/// * _cellSize: sets cellSize with given value.
		/// * _nbins: sets nbins with given value.
		/// * _derivAperture: sets derivAperture with given value.
		/// * _winSigma: sets winSigma with given value.
		/// * _histogramNormType: sets histogramNormType with given value.
		/// * _L2HysThreshold: sets L2HysThreshold with given value.
		/// * _gammaCorrection: sets gammaCorrection with given value.
		/// * _nlevels: sets nlevels with given value.
		/// * _signedGradient: sets signedGradient with given value.
		///
		/// ## Note
		/// This alternative version of [new] function uses the following default values for its arguments:
		/// * _deriv_aperture: 1
		/// * _win_sigma: -1
		/// * _histogram_norm_type: HOGDescriptor::L2Hys
		/// * _l2_hys_threshold: 0.2
		/// * _gamma_correction: false
		/// * _nlevels: HOGDescriptor::DEFAULT_NLEVELS
		/// * _signed_gradient: false
		#[inline]
		pub fn new_def(_win_size: core::Size, _block_size: core::Size, _block_stride: core::Size, _cell_size: core::Size, _nbins: i32) -> Result<crate::objdetect::HOGDescriptor> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_HOGDescriptor_HOGDescriptor_Size_Size_Size_Size_int(&_win_size, &_block_size, &_block_stride, &_cell_size, _nbins, ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			let ret = unsafe { crate::objdetect::HOGDescriptor::opencv_from_extern(ret) };
			Ok(ret)
		}

		/// Creates the HOG descriptor and detector with default parameters.
		///
		/// aqual to HOGDescriptor(Size(64,128), Size(16,16), Size(8,8), Size(8,8), 9 )
		///
		/// ## Overloaded parameters
		///
		///
		/// Creates the HOG descriptor and detector and loads HOGDescriptor parameters and coefficients for the linear SVM classifier from a file.
		/// ## Parameters
		/// * filename: The file name containing HOGDescriptor properties and coefficients for the linear SVM classifier.
		#[inline]
		pub fn new_from_file(filename: &str) -> Result<crate::objdetect::HOGDescriptor> {
			extern_container_arg!(filename);
			return_send!(via ocvrs_return);
			unsafe { sys::cv_HOGDescriptor_HOGDescriptor_const_StringR(filename.opencv_as_extern(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			let ret = unsafe { crate::objdetect::HOGDescriptor::opencv_from_extern(ret) };
			Ok(ret)
		}

		/// Creates the HOG descriptor and detector with default parameters.
		///
		/// aqual to HOGDescriptor(Size(64,128), Size(16,16), Size(8,8), Size(8,8), 9 )
		///
		/// ## Overloaded parameters
		///
		/// ## Parameters
		/// * d: the HOGDescriptor which cloned to create a new one.
		#[inline]
		pub fn copy(d: &impl crate::objdetect::HOGDescriptorTraitConst) -> Result<crate::objdetect::HOGDescriptor> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_HOGDescriptor_HOGDescriptor_const_HOGDescriptorR(d.as_raw_HOGDescriptor(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			let ret = unsafe { crate::objdetect::HOGDescriptor::opencv_from_extern(ret) };
			Ok(ret)
		}

		/// Returns coefficients of the classifier trained for people detection (for 64x128 windows).
		#[inline]
		pub fn get_default_people_detector() -> Result<core::Vector<f32>> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_HOGDescriptor_getDefaultPeopleDetector(ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			let ret = unsafe { core::Vector::<f32>::opencv_from_extern(ret) };
			Ok(ret)
		}

		/// @example samples/tapi/hog.cpp
		/// /
		/// Returns coefficients of the classifier trained for people detection (for 48x96 windows).
		#[inline]
		pub fn get_daimler_people_detector() -> Result<core::Vector<f32>> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_HOGDescriptor_getDaimlerPeopleDetector(ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			let ret = unsafe { core::Vector::<f32>::opencv_from_extern(ret) };
			Ok(ret)
		}

	}

	/// Constant methods for [crate::objdetect::HOGDescriptor]
	pub trait HOGDescriptorTraitConst {
		fn as_raw_HOGDescriptor(&self) -> *const c_void;

		/// Detection window size. Align to block size and block stride. Default value is Size(64,128).
		#[inline]
		fn win_size(&self) -> core::Size {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_HOGDescriptor_propWinSize_const(self.as_raw_HOGDescriptor(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			ret
		}

		/// Block size in pixels. Align to cell size. Default value is Size(16,16).
		#[inline]
		fn block_size(&self) -> core::Size {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_HOGDescriptor_propBlockSize_const(self.as_raw_HOGDescriptor(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			ret
		}

		/// Block stride. It must be a multiple of cell size. Default value is Size(8,8).
		#[inline]
		fn block_stride(&self) -> core::Size {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_HOGDescriptor_propBlockStride_const(self.as_raw_HOGDescriptor(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			ret
		}

		/// Cell size. Default value is Size(8,8).
		#[inline]
		fn cell_size(&self) -> core::Size {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_HOGDescriptor_propCellSize_const(self.as_raw_HOGDescriptor(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			ret
		}

		/// Number of bins used in the calculation of histogram of gradients. Default value is 9.
		#[inline]
		fn nbins(&self) -> i32 {
			let ret = unsafe { sys::cv_HOGDescriptor_propNbins_const(self.as_raw_HOGDescriptor()) };
			ret
		}

		/// not documented
		#[inline]
		fn deriv_aperture(&self) -> i32 {
			let ret = unsafe { sys::cv_HOGDescriptor_propDerivAperture_const(self.as_raw_HOGDescriptor()) };
			ret
		}

		/// Gaussian smoothing window parameter.
		#[inline]
		fn win_sigma(&self) -> f64 {
			let ret = unsafe { sys::cv_HOGDescriptor_propWinSigma_const(self.as_raw_HOGDescriptor()) };
			ret
		}

		/// histogramNormType
		#[inline]
		fn histogram_norm_type(&self) -> crate::objdetect::HOGDescriptor_HistogramNormType {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_HOGDescriptor_propHistogramNormType_const(self.as_raw_HOGDescriptor(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			ret
		}

		/// L2-Hys normalization method shrinkage.
		#[inline]
		fn l2_hys_threshold(&self) -> f64 {
			let ret = unsafe { sys::cv_HOGDescriptor_propL2HysThreshold_const(self.as_raw_HOGDescriptor()) };
			ret
		}

		/// Flag to specify whether the gamma correction preprocessing is required or not.
		#[inline]
		fn gamma_correction(&self) -> bool {
			let ret = unsafe { sys::cv_HOGDescriptor_propGammaCorrection_const(self.as_raw_HOGDescriptor()) };
			ret
		}

		/// coefficients for the linear SVM classifier.
		#[inline]
		fn svm_detector(&self) -> core::Vector<f32> {
			let ret = unsafe { sys::cv_HOGDescriptor_propSvmDetector_const(self.as_raw_HOGDescriptor()) };
			let ret = unsafe { core::Vector::<f32>::opencv_from_extern(ret) };
			ret
		}

		/// coefficients for the linear SVM classifier used when OpenCL is enabled
		#[inline]
		fn ocl_svm_detector(&self) -> core::UMat {
			let ret = unsafe { sys::cv_HOGDescriptor_propOclSvmDetector_const(self.as_raw_HOGDescriptor()) };
			let ret = unsafe { core::UMat::opencv_from_extern(ret) };
			ret
		}

		/// not documented
		#[inline]
		fn free_coef(&self) -> f32 {
			let ret = unsafe { sys::cv_HOGDescriptor_propFree_coef_const(self.as_raw_HOGDescriptor()) };
			ret
		}

		/// Maximum number of detection window increases. Default value is 64
		#[inline]
		fn nlevels(&self) -> i32 {
			let ret = unsafe { sys::cv_HOGDescriptor_propNlevels_const(self.as_raw_HOGDescriptor()) };
			ret
		}

		/// Indicates signed gradient will be used or not
		#[inline]
		fn signed_gradient(&self) -> bool {
			let ret = unsafe { sys::cv_HOGDescriptor_propSignedGradient_const(self.as_raw_HOGDescriptor()) };
			ret
		}

		/// Returns the number of coefficients required for the classification.
		#[inline]
		fn get_descriptor_size(&self) -> Result<size_t> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_HOGDescriptor_getDescriptorSize_const(self.as_raw_HOGDescriptor(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		/// Checks if detector size equal to descriptor size.
		#[inline]
		fn check_detector_size(&self) -> Result<bool> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_HOGDescriptor_checkDetectorSize_const(self.as_raw_HOGDescriptor(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		/// Returns winSigma value
		#[inline]
		fn get_win_sigma(&self) -> Result<f64> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_HOGDescriptor_getWinSigma_const(self.as_raw_HOGDescriptor(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		/// Stores HOGDescriptor parameters and coefficients for the linear SVM classifier in a file storage.
		/// ## Parameters
		/// * fs: File storage
		/// * objname: Object name
		#[inline]
		fn write(&self, fs: &mut impl core::FileStorageTrait, objname: &str) -> Result<()> {
			extern_container_arg!(objname);
			return_send!(via ocvrs_return);
			unsafe { sys::cv_HOGDescriptor_write_const_FileStorageR_const_StringR(self.as_raw_HOGDescriptor(), fs.as_raw_mut_FileStorage(), objname.opencv_as_extern(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		/// saves HOGDescriptor parameters and coefficients for the linear SVM classifier to a file
		/// ## Parameters
		/// * filename: File name
		/// * objname: Object name
		///
		/// ## C++ default parameters
		/// * objname: String()
		#[inline]
		fn save(&self, filename: &str, objname: &str) -> Result<()> {
			extern_container_arg!(filename);
			extern_container_arg!(objname);
			return_send!(via ocvrs_return);
			unsafe { sys::cv_HOGDescriptor_save_const_const_StringR_const_StringR(self.as_raw_HOGDescriptor(), filename.opencv_as_extern(), objname.opencv_as_extern(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		/// saves HOGDescriptor parameters and coefficients for the linear SVM classifier to a file
		/// ## Parameters
		/// * filename: File name
		/// * objname: Object name
		///
		/// ## Note
		/// This alternative version of [HOGDescriptorTraitConst::save] function uses the following default values for its arguments:
		/// * objname: String()
		#[inline]
		fn save_def(&self, filename: &str) -> Result<()> {
			extern_container_arg!(filename);
			return_send!(via ocvrs_return);
			unsafe { sys::cv_HOGDescriptor_save_const_const_StringR(self.as_raw_HOGDescriptor(), filename.opencv_as_extern(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		/// clones the HOGDescriptor
		/// ## Parameters
		/// * c: cloned HOGDescriptor
		#[inline]
		fn copy_to(&self, c: &mut impl crate::objdetect::HOGDescriptorTrait) -> Result<()> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_HOGDescriptor_copyTo_const_HOGDescriptorR(self.as_raw_HOGDescriptor(), c.as_raw_mut_HOGDescriptor(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		/// @example samples/cpp/train_HOG.cpp
		/// /
		/// Computes HOG descriptors of given image.
		/// ## Parameters
		/// * img: Matrix of the type CV_8U containing an image where HOG features will be calculated.
		/// * descriptors: Matrix of the type CV_32F
		/// * winStride: Window stride. It must be a multiple of block stride.
		/// * padding: Padding
		/// * locations: Vector of Point
		///
		/// ## C++ default parameters
		/// * win_stride: Size()
		/// * padding: Size()
		/// * locations: std::vector<Point>()
		#[inline]
		fn compute(&self, img: &impl ToInputArray, descriptors: &mut core::Vector<f32>, win_stride: core::Size, padding: core::Size, locations: &core::Vector<core::Point>) -> Result<()> {
			input_array_arg!(img);
			return_send!(via ocvrs_return);
			unsafe { sys::cv_HOGDescriptor_compute_const_const__InputArrayR_vectorLfloatGR_Size_Size_const_vectorLPointGR(self.as_raw_HOGDescriptor(), img.as_raw__InputArray(), descriptors.as_raw_mut_VectorOff32(), &win_stride, &padding, locations.as_raw_VectorOfPoint(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		/// @example samples/cpp/train_HOG.cpp
		/// /
		/// Computes HOG descriptors of given image.
		/// ## Parameters
		/// * img: Matrix of the type CV_8U containing an image where HOG features will be calculated.
		/// * descriptors: Matrix of the type CV_32F
		/// * winStride: Window stride. It must be a multiple of block stride.
		/// * padding: Padding
		/// * locations: Vector of Point
		///
		/// ## Note
		/// This alternative version of [HOGDescriptorTraitConst::compute] function uses the following default values for its arguments:
		/// * win_stride: Size()
		/// * padding: Size()
		/// * locations: std::vector<Point>()
		#[inline]
		fn compute_def(&self, img: &impl ToInputArray, descriptors: &mut core::Vector<f32>) -> Result<()> {
			input_array_arg!(img);
			return_send!(via ocvrs_return);
			unsafe { sys::cv_HOGDescriptor_compute_const_const__InputArrayR_vectorLfloatGR(self.as_raw_HOGDescriptor(), img.as_raw__InputArray(), descriptors.as_raw_mut_VectorOff32(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		/// Performs object detection without a multi-scale window.
		/// ## Parameters
		/// * img: Matrix of the type CV_8U or CV_8UC3 containing an image where objects are detected.
		/// * foundLocations: Vector of point where each point contains left-top corner point of detected object boundaries.
		/// * weights: Vector that will contain confidence values for each detected object.
		/// * hitThreshold: Threshold for the distance between features and SVM classifying plane.
		/// Usually it is 0 and should be specified in the detector coefficients (as the last free coefficient).
		/// But if the free coefficient is omitted (which is allowed), you can specify it manually here.
		/// * winStride: Window stride. It must be a multiple of block stride.
		/// * padding: Padding
		/// * searchLocations: Vector of Point includes set of requested locations to be evaluated.
		///
		/// ## C++ default parameters
		/// * hit_threshold: 0
		/// * win_stride: Size()
		/// * padding: Size()
		/// * search_locations: std::vector<Point>()
		#[inline]
		fn detect_weights(&self, img: &impl ToInputArray, found_locations: &mut core::Vector<core::Point>, weights: &mut core::Vector<f64>, hit_threshold: f64, win_stride: core::Size, padding: core::Size, search_locations: &core::Vector<core::Point>) -> Result<()> {
			input_array_arg!(img);
			return_send!(via ocvrs_return);
			unsafe { sys::cv_HOGDescriptor_detect_const_const__InputArrayR_vectorLPointGR_vectorLdoubleGR_double_Size_Size_const_vectorLPointGR(self.as_raw_HOGDescriptor(), img.as_raw__InputArray(), found_locations.as_raw_mut_VectorOfPoint(), weights.as_raw_mut_VectorOff64(), hit_threshold, &win_stride, &padding, search_locations.as_raw_VectorOfPoint(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		/// Performs object detection without a multi-scale window.
		/// ## Parameters
		/// * img: Matrix of the type CV_8U or CV_8UC3 containing an image where objects are detected.
		/// * foundLocations: Vector of point where each point contains left-top corner point of detected object boundaries.
		/// * weights: Vector that will contain confidence values for each detected object.
		/// * hitThreshold: Threshold for the distance between features and SVM classifying plane.
		/// Usually it is 0 and should be specified in the detector coefficients (as the last free coefficient).
		/// But if the free coefficient is omitted (which is allowed), you can specify it manually here.
		/// * winStride: Window stride. It must be a multiple of block stride.
		/// * padding: Padding
		/// * searchLocations: Vector of Point includes set of requested locations to be evaluated.
		///
		/// ## Note
		/// This alternative version of [HOGDescriptorTraitConst::detect_weights] function uses the following default values for its arguments:
		/// * hit_threshold: 0
		/// * win_stride: Size()
		/// * padding: Size()
		/// * search_locations: std::vector<Point>()
		#[inline]
		fn detect_weights_def(&self, img: &impl ToInputArray, found_locations: &mut core::Vector<core::Point>, weights: &mut core::Vector<f64>) -> Result<()> {
			input_array_arg!(img);
			return_send!(via ocvrs_return);
			unsafe { sys::cv_HOGDescriptor_detect_const_const__InputArrayR_vectorLPointGR_vectorLdoubleGR(self.as_raw_HOGDescriptor(), img.as_raw__InputArray(), found_locations.as_raw_mut_VectorOfPoint(), weights.as_raw_mut_VectorOff64(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		/// Performs object detection without a multi-scale window.
		/// ## Parameters
		/// * img: Matrix of the type CV_8U or CV_8UC3 containing an image where objects are detected.
		/// * foundLocations: Vector of point where each point contains left-top corner point of detected object boundaries.
		/// * hitThreshold: Threshold for the distance between features and SVM classifying plane.
		/// Usually it is 0 and should be specified in the detector coefficients (as the last free coefficient).
		/// But if the free coefficient is omitted (which is allowed), you can specify it manually here.
		/// * winStride: Window stride. It must be a multiple of block stride.
		/// * padding: Padding
		/// * searchLocations: Vector of Point includes locations to search.
		///
		/// ## C++ default parameters
		/// * hit_threshold: 0
		/// * win_stride: Size()
		/// * padding: Size()
		/// * search_locations: std::vector<Point>()
		#[inline]
		fn detect(&self, img: &impl ToInputArray, found_locations: &mut core::Vector<core::Point>, hit_threshold: f64, win_stride: core::Size, padding: core::Size, search_locations: &core::Vector<core::Point>) -> Result<()> {
			input_array_arg!(img);
			return_send!(via ocvrs_return);
			unsafe { sys::cv_HOGDescriptor_detect_const_const__InputArrayR_vectorLPointGR_double_Size_Size_const_vectorLPointGR(self.as_raw_HOGDescriptor(), img.as_raw__InputArray(), found_locations.as_raw_mut_VectorOfPoint(), hit_threshold, &win_stride, &padding, search_locations.as_raw_VectorOfPoint(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		/// Performs object detection without a multi-scale window.
		/// ## Parameters
		/// * img: Matrix of the type CV_8U or CV_8UC3 containing an image where objects are detected.
		/// * foundLocations: Vector of point where each point contains left-top corner point of detected object boundaries.
		/// * hitThreshold: Threshold for the distance between features and SVM classifying plane.
		/// Usually it is 0 and should be specified in the detector coefficients (as the last free coefficient).
		/// But if the free coefficient is omitted (which is allowed), you can specify it manually here.
		/// * winStride: Window stride. It must be a multiple of block stride.
		/// * padding: Padding
		/// * searchLocations: Vector of Point includes locations to search.
		///
		/// ## Note
		/// This alternative version of [HOGDescriptorTraitConst::detect] function uses the following default values for its arguments:
		/// * hit_threshold: 0
		/// * win_stride: Size()
		/// * padding: Size()
		/// * search_locations: std::vector<Point>()
		#[inline]
		fn detect_def(&self, img: &impl ToInputArray, found_locations: &mut core::Vector<core::Point>) -> Result<()> {
			input_array_arg!(img);
			return_send!(via ocvrs_return);
			unsafe { sys::cv_HOGDescriptor_detect_const_const__InputArrayR_vectorLPointGR(self.as_raw_HOGDescriptor(), img.as_raw__InputArray(), found_locations.as_raw_mut_VectorOfPoint(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		/// Detects objects of different sizes in the input image. The detected objects are returned as a list
		/// of rectangles.
		/// ## Parameters
		/// * img: Matrix of the type CV_8U or CV_8UC3 containing an image where objects are detected.
		/// * foundLocations: Vector of rectangles where each rectangle contains the detected object.
		/// * foundWeights: Vector that will contain confidence values for each detected object.
		/// * hitThreshold: Threshold for the distance between features and SVM classifying plane.
		/// Usually it is 0 and should be specified in the detector coefficients (as the last free coefficient).
		/// But if the free coefficient is omitted (which is allowed), you can specify it manually here.
		/// * winStride: Window stride. It must be a multiple of block stride.
		/// * padding: Padding
		/// * scale: Coefficient of the detection window increase.
		/// * groupThreshold: Coefficient to regulate the similarity threshold. When detected, some objects can be covered
		/// by many rectangles. 0 means not to perform grouping.
		/// * useMeanshiftGrouping: indicates grouping algorithm
		///
		/// ## C++ default parameters
		/// * hit_threshold: 0
		/// * win_stride: Size()
		/// * padding: Size()
		/// * scale: 1.05
		/// * group_threshold: 2.0
		/// * use_meanshift_grouping: false
		#[inline]
		fn detect_multi_scale_weights(&self, img: &impl ToInputArray, found_locations: &mut core::Vector<core::Rect>, found_weights: &mut core::Vector<f64>, hit_threshold: f64, win_stride: core::Size, padding: core::Size, scale: f64, group_threshold: f64, use_meanshift_grouping: bool) -> Result<()> {
			input_array_arg!(img);
			return_send!(via ocvrs_return);
			unsafe { sys::cv_HOGDescriptor_detectMultiScale_const_const__InputArrayR_vectorLRectGR_vectorLdoubleGR_double_Size_Size_double_double_bool(self.as_raw_HOGDescriptor(), img.as_raw__InputArray(), found_locations.as_raw_mut_VectorOfRect(), found_weights.as_raw_mut_VectorOff64(), hit_threshold, &win_stride, &padding, scale, group_threshold, use_meanshift_grouping, ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		/// Detects objects of different sizes in the input image. The detected objects are returned as a list
		/// of rectangles.
		/// ## Parameters
		/// * img: Matrix of the type CV_8U or CV_8UC3 containing an image where objects are detected.
		/// * foundLocations: Vector of rectangles where each rectangle contains the detected object.
		/// * foundWeights: Vector that will contain confidence values for each detected object.
		/// * hitThreshold: Threshold for the distance between features and SVM classifying plane.
		/// Usually it is 0 and should be specified in the detector coefficients (as the last free coefficient).
		/// But if the free coefficient is omitted (which is allowed), you can specify it manually here.
		/// * winStride: Window stride. It must be a multiple of block stride.
		/// * padding: Padding
		/// * scale: Coefficient of the detection window increase.
		/// * groupThreshold: Coefficient to regulate the similarity threshold. When detected, some objects can be covered
		/// by many rectangles. 0 means not to perform grouping.
		/// * useMeanshiftGrouping: indicates grouping algorithm
		///
		/// ## Note
		/// This alternative version of [HOGDescriptorTraitConst::detect_multi_scale_weights] function uses the following default values for its arguments:
		/// * hit_threshold: 0
		/// * win_stride: Size()
		/// * padding: Size()
		/// * scale: 1.05
		/// * group_threshold: 2.0
		/// * use_meanshift_grouping: false
		#[inline]
		fn detect_multi_scale_weights_def(&self, img: &impl ToInputArray, found_locations: &mut core::Vector<core::Rect>, found_weights: &mut core::Vector<f64>) -> Result<()> {
			input_array_arg!(img);
			return_send!(via ocvrs_return);
			unsafe { sys::cv_HOGDescriptor_detectMultiScale_const_const__InputArrayR_vectorLRectGR_vectorLdoubleGR(self.as_raw_HOGDescriptor(), img.as_raw__InputArray(), found_locations.as_raw_mut_VectorOfRect(), found_weights.as_raw_mut_VectorOff64(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		/// Detects objects of different sizes in the input image. The detected objects are returned as a list
		/// of rectangles.
		/// ## Parameters
		/// * img: Matrix of the type CV_8U or CV_8UC3 containing an image where objects are detected.
		/// * foundLocations: Vector of rectangles where each rectangle contains the detected object.
		/// * hitThreshold: Threshold for the distance between features and SVM classifying plane.
		/// Usually it is 0 and should be specified in the detector coefficients (as the last free coefficient).
		/// But if the free coefficient is omitted (which is allowed), you can specify it manually here.
		/// * winStride: Window stride. It must be a multiple of block stride.
		/// * padding: Padding
		/// * scale: Coefficient of the detection window increase.
		/// * groupThreshold: Coefficient to regulate the similarity threshold. When detected, some objects can be covered
		/// by many rectangles. 0 means not to perform grouping.
		/// * useMeanshiftGrouping: indicates grouping algorithm
		///
		/// ## C++ default parameters
		/// * hit_threshold: 0
		/// * win_stride: Size()
		/// * padding: Size()
		/// * scale: 1.05
		/// * group_threshold: 2.0
		/// * use_meanshift_grouping: false
		#[inline]
		fn detect_multi_scale(&self, img: &impl ToInputArray, found_locations: &mut core::Vector<core::Rect>, hit_threshold: f64, win_stride: core::Size, padding: core::Size, scale: f64, group_threshold: f64, use_meanshift_grouping: bool) -> Result<()> {
			input_array_arg!(img);
			return_send!(via ocvrs_return);
			unsafe { sys::cv_HOGDescriptor_detectMultiScale_const_const__InputArrayR_vectorLRectGR_double_Size_Size_double_double_bool(self.as_raw_HOGDescriptor(), img.as_raw__InputArray(), found_locations.as_raw_mut_VectorOfRect(), hit_threshold, &win_stride, &padding, scale, group_threshold, use_meanshift_grouping, ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		/// Detects objects of different sizes in the input image. The detected objects are returned as a list
		/// of rectangles.
		/// ## Parameters
		/// * img: Matrix of the type CV_8U or CV_8UC3 containing an image where objects are detected.
		/// * foundLocations: Vector of rectangles where each rectangle contains the detected object.
		/// * hitThreshold: Threshold for the distance between features and SVM classifying plane.
		/// Usually it is 0 and should be specified in the detector coefficients (as the last free coefficient).
		/// But if the free coefficient is omitted (which is allowed), you can specify it manually here.
		/// * winStride: Window stride. It must be a multiple of block stride.
		/// * padding: Padding
		/// * scale: Coefficient of the detection window increase.
		/// * groupThreshold: Coefficient to regulate the similarity threshold. When detected, some objects can be covered
		/// by many rectangles. 0 means not to perform grouping.
		/// * useMeanshiftGrouping: indicates grouping algorithm
		///
		/// ## Note
		/// This alternative version of [HOGDescriptorTraitConst::detect_multi_scale] function uses the following default values for its arguments:
		/// * hit_threshold: 0
		/// * win_stride: Size()
		/// * padding: Size()
		/// * scale: 1.05
		/// * group_threshold: 2.0
		/// * use_meanshift_grouping: false
		#[inline]
		fn detect_multi_scale_def(&self, img: &impl ToInputArray, found_locations: &mut core::Vector<core::Rect>) -> Result<()> {
			input_array_arg!(img);
			return_send!(via ocvrs_return);
			unsafe { sys::cv_HOGDescriptor_detectMultiScale_const_const__InputArrayR_vectorLRectGR(self.as_raw_HOGDescriptor(), img.as_raw__InputArray(), found_locations.as_raw_mut_VectorOfRect(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		/// Computes gradients and quantized gradient orientations.
		/// ## Parameters
		/// * img: Matrix contains the image to be computed
		/// * grad: Matrix of type CV_32FC2 contains computed gradients
		/// * angleOfs: Matrix of type CV_8UC2 contains quantized gradient orientations
		/// * paddingTL: Padding from top-left
		/// * paddingBR: Padding from bottom-right
		///
		/// ## C++ default parameters
		/// * padding_tl: Size()
		/// * padding_br: Size()
		#[inline]
		fn compute_gradient(&self, img: &impl ToInputArray, grad: &mut impl ToInputOutputArray, angle_ofs: &mut impl ToInputOutputArray, padding_tl: core::Size, padding_br: core::Size) -> Result<()> {
			input_array_arg!(img);
			input_output_array_arg!(grad);
			input_output_array_arg!(angle_ofs);
			return_send!(via ocvrs_return);
			unsafe { sys::cv_HOGDescriptor_computeGradient_const_const__InputArrayR_const__InputOutputArrayR_const__InputOutputArrayR_Size_Size(self.as_raw_HOGDescriptor(), img.as_raw__InputArray(), grad.as_raw__InputOutputArray(), angle_ofs.as_raw__InputOutputArray(), &padding_tl, &padding_br, ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		/// Computes gradients and quantized gradient orientations.
		/// ## Parameters
		/// * img: Matrix contains the image to be computed
		/// * grad: Matrix of type CV_32FC2 contains computed gradients
		/// * angleOfs: Matrix of type CV_8UC2 contains quantized gradient orientations
		/// * paddingTL: Padding from top-left
		/// * paddingBR: Padding from bottom-right
		///
		/// ## Note
		/// This alternative version of [HOGDescriptorTraitConst::compute_gradient] function uses the following default values for its arguments:
		/// * padding_tl: Size()
		/// * padding_br: Size()
		#[inline]
		fn compute_gradient_def(&self, img: &impl ToInputArray, grad: &mut impl ToInputOutputArray, angle_ofs: &mut impl ToInputOutputArray) -> Result<()> {
			input_array_arg!(img);
			input_output_array_arg!(grad);
			input_output_array_arg!(angle_ofs);
			return_send!(via ocvrs_return);
			unsafe { sys::cv_HOGDescriptor_computeGradient_const_const__InputArrayR_const__InputOutputArrayR_const__InputOutputArrayR(self.as_raw_HOGDescriptor(), img.as_raw__InputArray(), grad.as_raw__InputOutputArray(), angle_ofs.as_raw__InputOutputArray(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		/// evaluate specified ROI and return confidence value for each location
		/// ## Parameters
		/// * img: Matrix of the type CV_8U or CV_8UC3 containing an image where objects are detected.
		/// * locations: Vector of Point
		/// * foundLocations: Vector of Point where each Point is detected object's top-left point.
		/// * confidences: confidences
		/// * hitThreshold: Threshold for the distance between features and SVM classifying plane. Usually
		/// it is 0 and should be specified in the detector coefficients (as the last free coefficient). But if
		/// the free coefficient is omitted (which is allowed), you can specify it manually here
		/// * winStride: winStride
		/// * padding: padding
		///
		/// ## C++ default parameters
		/// * hit_threshold: 0
		/// * win_stride: Size()
		/// * padding: Size()
		#[inline]
		fn detect_roi(&self, img: &impl ToInputArray, locations: &core::Vector<core::Point>, found_locations: &mut core::Vector<core::Point>, confidences: &mut core::Vector<f64>, hit_threshold: f64, win_stride: core::Size, padding: core::Size) -> Result<()> {
			input_array_arg!(img);
			return_send!(via ocvrs_return);
			unsafe { sys::cv_HOGDescriptor_detectROI_const_const__InputArrayR_const_vectorLPointGR_vectorLPointGR_vectorLdoubleGR_double_Size_Size(self.as_raw_HOGDescriptor(), img.as_raw__InputArray(), locations.as_raw_VectorOfPoint(), found_locations.as_raw_mut_VectorOfPoint(), confidences.as_raw_mut_VectorOff64(), hit_threshold, &win_stride, &padding, ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		/// evaluate specified ROI and return confidence value for each location
		/// ## Parameters
		/// * img: Matrix of the type CV_8U or CV_8UC3 containing an image where objects are detected.
		/// * locations: Vector of Point
		/// * foundLocations: Vector of Point where each Point is detected object's top-left point.
		/// * confidences: confidences
		/// * hitThreshold: Threshold for the distance between features and SVM classifying plane. Usually
		/// it is 0 and should be specified in the detector coefficients (as the last free coefficient). But if
		/// the free coefficient is omitted (which is allowed), you can specify it manually here
		/// * winStride: winStride
		/// * padding: padding
		///
		/// ## Note
		/// This alternative version of [HOGDescriptorTraitConst::detect_roi] function uses the following default values for its arguments:
		/// * hit_threshold: 0
		/// * win_stride: Size()
		/// * padding: Size()
		#[inline]
		fn detect_roi_def(&self, img: &impl ToInputArray, locations: &core::Vector<core::Point>, found_locations: &mut core::Vector<core::Point>, confidences: &mut core::Vector<f64>) -> Result<()> {
			input_array_arg!(img);
			return_send!(via ocvrs_return);
			unsafe { sys::cv_HOGDescriptor_detectROI_const_const__InputArrayR_const_vectorLPointGR_vectorLPointGR_vectorLdoubleGR(self.as_raw_HOGDescriptor(), img.as_raw__InputArray(), locations.as_raw_VectorOfPoint(), found_locations.as_raw_mut_VectorOfPoint(), confidences.as_raw_mut_VectorOff64(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		/// evaluate specified ROI and return confidence value for each location in multiple scales
		/// ## Parameters
		/// * img: Matrix of the type CV_8U or CV_8UC3 containing an image where objects are detected.
		/// * foundLocations: Vector of rectangles where each rectangle contains the detected object.
		/// * locations: Vector of DetectionROI
		/// * hitThreshold: Threshold for the distance between features and SVM classifying plane. Usually it is 0 and should be specified
		/// in the detector coefficients (as the last free coefficient). But if the free coefficient is omitted (which is allowed), you can specify it manually here.
		/// * groupThreshold: Minimum possible number of rectangles minus 1. The threshold is used in a group of rectangles to retain it.
		///
		/// ## C++ default parameters
		/// * hit_threshold: 0
		/// * group_threshold: 0
		#[inline]
		fn detect_multi_scale_roi(&self, img: &impl ToInputArray, found_locations: &mut core::Vector<core::Rect>, locations: &mut core::Vector<crate::objdetect::DetectionROI>, hit_threshold: f64, group_threshold: i32) -> Result<()> {
			input_array_arg!(img);
			return_send!(via ocvrs_return);
			unsafe { sys::cv_HOGDescriptor_detectMultiScaleROI_const_const__InputArrayR_vectorLRectGR_vectorLDetectionROIGR_double_int(self.as_raw_HOGDescriptor(), img.as_raw__InputArray(), found_locations.as_raw_mut_VectorOfRect(), locations.as_raw_mut_VectorOfDetectionROI(), hit_threshold, group_threshold, ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		/// evaluate specified ROI and return confidence value for each location in multiple scales
		/// ## Parameters
		/// * img: Matrix of the type CV_8U or CV_8UC3 containing an image where objects are detected.
		/// * foundLocations: Vector of rectangles where each rectangle contains the detected object.
		/// * locations: Vector of DetectionROI
		/// * hitThreshold: Threshold for the distance between features and SVM classifying plane. Usually it is 0 and should be specified
		/// in the detector coefficients (as the last free coefficient). But if the free coefficient is omitted (which is allowed), you can specify it manually here.
		/// * groupThreshold: Minimum possible number of rectangles minus 1. The threshold is used in a group of rectangles to retain it.
		///
		/// ## Note
		/// This alternative version of [HOGDescriptorTraitConst::detect_multi_scale_roi] function uses the following default values for its arguments:
		/// * hit_threshold: 0
		/// * group_threshold: 0
		#[inline]
		fn detect_multi_scale_roi_def(&self, img: &impl ToInputArray, found_locations: &mut core::Vector<core::Rect>, locations: &mut core::Vector<crate::objdetect::DetectionROI>) -> Result<()> {
			input_array_arg!(img);
			return_send!(via ocvrs_return);
			unsafe { sys::cv_HOGDescriptor_detectMultiScaleROI_const_const__InputArrayR_vectorLRectGR_vectorLDetectionROIGR(self.as_raw_HOGDescriptor(), img.as_raw__InputArray(), found_locations.as_raw_mut_VectorOfRect(), locations.as_raw_mut_VectorOfDetectionROI(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		/// Groups the object candidate rectangles.
		/// ## Parameters
		/// * rectList: Input/output vector of rectangles. Output vector includes retained and grouped rectangles. (The Python list is not modified in place.)
		/// * weights: Input/output vector of weights of rectangles. Output vector includes weights of retained and grouped rectangles. (The Python list is not modified in place.)
		/// * groupThreshold: Minimum possible number of rectangles minus 1. The threshold is used in a group of rectangles to retain it.
		/// * eps: Relative difference between sides of the rectangles to merge them into a group.
		#[inline]
		fn group_rectangles(&self, rect_list: &mut core::Vector<core::Rect>, weights: &mut core::Vector<f64>, group_threshold: i32, eps: f64) -> Result<()> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_HOGDescriptor_groupRectangles_const_vectorLRectGR_vectorLdoubleGR_int_double(self.as_raw_HOGDescriptor(), rect_list.as_raw_mut_VectorOfRect(), weights.as_raw_mut_VectorOff64(), group_threshold, eps, ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

	}

	/// Mutable methods for [crate::objdetect::HOGDescriptor]
	pub trait HOGDescriptorTrait: crate::objdetect::HOGDescriptorTraitConst {
		fn as_raw_mut_HOGDescriptor(&mut self) -> *mut c_void;

		/// Detection window size. Align to block size and block stride. Default value is Size(64,128).
		#[inline]
		fn set_win_size(&mut self, val: core::Size) {
			let ret = unsafe { sys::cv_HOGDescriptor_propWinSize_const_Size(self.as_raw_mut_HOGDescriptor(), &val) };
			ret
		}

		/// Block size in pixels. Align to cell size. Default value is Size(16,16).
		#[inline]
		fn set_block_size(&mut self, val: core::Size) {
			let ret = unsafe { sys::cv_HOGDescriptor_propBlockSize_const_Size(self.as_raw_mut_HOGDescriptor(), &val) };
			ret
		}

		/// Block stride. It must be a multiple of cell size. Default value is Size(8,8).
		#[inline]
		fn set_block_stride(&mut self, val: core::Size) {
			let ret = unsafe { sys::cv_HOGDescriptor_propBlockStride_const_Size(self.as_raw_mut_HOGDescriptor(), &val) };
			ret
		}

		/// Cell size. Default value is Size(8,8).
		#[inline]
		fn set_cell_size(&mut self, val: core::Size) {
			let ret = unsafe { sys::cv_HOGDescriptor_propCellSize_const_Size(self.as_raw_mut_HOGDescriptor(), &val) };
			ret
		}

		/// Number of bins used in the calculation of histogram of gradients. Default value is 9.
		#[inline]
		fn set_nbins(&mut self, val: i32) {
			let ret = unsafe { sys::cv_HOGDescriptor_propNbins_const_int(self.as_raw_mut_HOGDescriptor(), val) };
			ret
		}

		/// not documented
		#[inline]
		fn set_deriv_aperture(&mut self, val: i32) {
			let ret = unsafe { sys::cv_HOGDescriptor_propDerivAperture_const_int(self.as_raw_mut_HOGDescriptor(), val) };
			ret
		}

		/// Gaussian smoothing window parameter.
		#[inline]
		fn set_win_sigma(&mut self, val: f64) {
			let ret = unsafe { sys::cv_HOGDescriptor_propWinSigma_const_double(self.as_raw_mut_HOGDescriptor(), val) };
			ret
		}

		/// histogramNormType
		#[inline]
		fn set_histogram_norm_type(&mut self, val: crate::objdetect::HOGDescriptor_HistogramNormType) {
			let ret = unsafe { sys::cv_HOGDescriptor_propHistogramNormType_const_HistogramNormType(self.as_raw_mut_HOGDescriptor(), val) };
			ret
		}

		/// L2-Hys normalization method shrinkage.
		#[inline]
		fn set_l2_hys_threshold(&mut self, val: f64) {
			let ret = unsafe { sys::cv_HOGDescriptor_propL2HysThreshold_const_double(self.as_raw_mut_HOGDescriptor(), val) };
			ret
		}

		/// Flag to specify whether the gamma correction preprocessing is required or not.
		#[inline]
		fn set_gamma_correction(&mut self, val: bool) {
			let ret = unsafe { sys::cv_HOGDescriptor_propGammaCorrection_const_bool(self.as_raw_mut_HOGDescriptor(), val) };
			ret
		}

		/// coefficients for the linear SVM classifier.
		#[inline]
		fn set_svm_detector(&mut self, val: core::Vector<f32>) {
			let ret = unsafe { sys::cv_HOGDescriptor_propSvmDetector_const_vectorLfloatG(self.as_raw_mut_HOGDescriptor(), val.as_raw_VectorOff32()) };
			ret
		}

		/// coefficients for the linear SVM classifier used when OpenCL is enabled
		#[inline]
		fn set_ocl_svm_detector(&mut self, val: core::UMat) {
			let ret = unsafe { sys::cv_HOGDescriptor_propOclSvmDetector_const_UMat(self.as_raw_mut_HOGDescriptor(), val.as_raw_UMat()) };
			ret
		}

		/// not documented
		#[inline]
		fn set_free_coef(&mut self, val: f32) {
			let ret = unsafe { sys::cv_HOGDescriptor_propFree_coef_const_float(self.as_raw_mut_HOGDescriptor(), val) };
			ret
		}

		/// Maximum number of detection window increases. Default value is 64
		#[inline]
		fn set_nlevels(&mut self, val: i32) {
			let ret = unsafe { sys::cv_HOGDescriptor_propNlevels_const_int(self.as_raw_mut_HOGDescriptor(), val) };
			ret
		}

		/// Indicates signed gradient will be used or not
		#[inline]
		fn set_signed_gradient(&mut self, val: bool) {
			let ret = unsafe { sys::cv_HOGDescriptor_propSignedGradient_const_bool(self.as_raw_mut_HOGDescriptor(), val) };
			ret
		}

		/// @example samples/cpp/peopledetect.cpp
		/// /
		/// Sets coefficients for the linear SVM classifier.
		/// ## Parameters
		/// * svmdetector: coefficients for the linear SVM classifier.
		#[inline]
		fn set_svm_detector_input_array(&mut self, svmdetector: &impl ToInputArray) -> Result<()> {
			input_array_arg!(svmdetector);
			return_send!(via ocvrs_return);
			unsafe { sys::cv_HOGDescriptor_setSVMDetector_const__InputArrayR(self.as_raw_mut_HOGDescriptor(), svmdetector.as_raw__InputArray(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		/// Reads HOGDescriptor parameters and coefficients for the linear SVM classifier from a file node.
		/// ## Parameters
		/// * fn: File node
		#[inline]
		fn read(&mut self, fn_: &mut impl core::FileNodeTrait) -> Result<bool> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_HOGDescriptor_read_FileNodeR(self.as_raw_mut_HOGDescriptor(), fn_.as_raw_mut_FileNode(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		/// loads HOGDescriptor parameters and coefficients for the linear SVM classifier from a file
		/// ## Parameters
		/// * filename: Name of the file to read.
		/// * objname: The optional name of the node to read (if empty, the first top-level node will be used).
		///
		/// ## C++ default parameters
		/// * objname: String()
		#[inline]
		fn load(&mut self, filename: &str, objname: &str) -> Result<bool> {
			extern_container_arg!(filename);
			extern_container_arg!(objname);
			return_send!(via ocvrs_return);
			unsafe { sys::cv_HOGDescriptor_load_const_StringR_const_StringR(self.as_raw_mut_HOGDescriptor(), filename.opencv_as_extern(), objname.opencv_as_extern(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		/// loads HOGDescriptor parameters and coefficients for the linear SVM classifier from a file
		/// ## Parameters
		/// * filename: Name of the file to read.
		/// * objname: The optional name of the node to read (if empty, the first top-level node will be used).
		///
		/// ## Note
		/// This alternative version of [HOGDescriptorTrait::load] function uses the following default values for its arguments:
		/// * objname: String()
		#[inline]
		fn load_def(&mut self, filename: &str) -> Result<bool> {
			extern_container_arg!(filename);
			return_send!(via ocvrs_return);
			unsafe { sys::cv_HOGDescriptor_load_const_StringR(self.as_raw_mut_HOGDescriptor(), filename.opencv_as_extern(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

	}

	impl std::fmt::Debug for HOGDescriptor {
		#[inline]
		fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
			f.debug_struct("HOGDescriptor")
				.field("win_size", &crate::objdetect::HOGDescriptorTraitConst::win_size(self))
				.field("block_size", &crate::objdetect::HOGDescriptorTraitConst::block_size(self))
				.field("block_stride", &crate::objdetect::HOGDescriptorTraitConst::block_stride(self))
				.field("cell_size", &crate::objdetect::HOGDescriptorTraitConst::cell_size(self))
				.field("nbins", &crate::objdetect::HOGDescriptorTraitConst::nbins(self))
				.field("deriv_aperture", &crate::objdetect::HOGDescriptorTraitConst::deriv_aperture(self))
				.field("win_sigma", &crate::objdetect::HOGDescriptorTraitConst::win_sigma(self))
				.field("histogram_norm_type", &crate::objdetect::HOGDescriptorTraitConst::histogram_norm_type(self))
				.field("l2_hys_threshold", &crate::objdetect::HOGDescriptorTraitConst::l2_hys_threshold(self))
				.field("gamma_correction", &crate::objdetect::HOGDescriptorTraitConst::gamma_correction(self))
				.field("svm_detector", &crate::objdetect::HOGDescriptorTraitConst::svm_detector(self))
				.field("ocl_svm_detector", &crate::objdetect::HOGDescriptorTraitConst::ocl_svm_detector(self))
				.field("free_coef", &crate::objdetect::HOGDescriptorTraitConst::free_coef(self))
				.field("nlevels", &crate::objdetect::HOGDescriptorTraitConst::nlevels(self))
				.field("signed_gradient", &crate::objdetect::HOGDescriptorTraitConst::signed_gradient(self))
				.finish()
		}
	}

	impl crate::objdetect::HOGDescriptorTraitConst for HOGDescriptor {
		#[inline] fn as_raw_HOGDescriptor(&self) -> *const c_void { self.as_raw() }
	}

	impl crate::objdetect::HOGDescriptorTrait for HOGDescriptor {
		#[inline] fn as_raw_mut_HOGDescriptor(&mut self) -> *mut c_void { self.as_raw_mut() }
	}

	boxed_ref! { HOGDescriptor, crate::objdetect::HOGDescriptorTraitConst, as_raw_HOGDescriptor, crate::objdetect::HOGDescriptorTrait, as_raw_mut_HOGDescriptor }

	pub struct QRCodeDetector {
		ptr: *mut c_void,
	}

	opencv_type_boxed! { QRCodeDetector }

	impl Drop for QRCodeDetector {
		#[inline]
		fn drop(&mut self) {
			unsafe { sys::cv_QRCodeDetector_delete(self.as_raw_mut_QRCodeDetector()) };
		}
	}

	unsafe impl Send for QRCodeDetector {}

	impl QRCodeDetector {
		#[inline]
		pub fn default() -> Result<crate::objdetect::QRCodeDetector> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_QRCodeDetector_QRCodeDetector(ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			let ret = unsafe { crate::objdetect::QRCodeDetector::opencv_from_extern(ret) };
			Ok(ret)
		}

	}

	/// Constant methods for [crate::objdetect::QRCodeDetector]
	pub trait QRCodeDetectorTraitConst {
		fn as_raw_QRCodeDetector(&self) -> *const c_void;

		/// Detects QR code in image and returns the quadrangle containing the code.
		/// ## Parameters
		/// * img: grayscale or color (BGR) image containing (or not) QR code.
		/// * points: Output vector of vertices of the minimum-area quadrangle containing the code.
		#[inline]
		fn detect(&self, img: &impl ToInputArray, points: &mut impl ToOutputArray) -> Result<bool> {
			input_array_arg!(img);
			output_array_arg!(points);
			return_send!(via ocvrs_return);
			unsafe { sys::cv_QRCodeDetector_detect_const_const__InputArrayR_const__OutputArrayR(self.as_raw_QRCodeDetector(), img.as_raw__InputArray(), points.as_raw__OutputArray(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		/// Detects QR codes in image and returns the vector of the quadrangles containing the codes.
		/// ## Parameters
		/// * img: grayscale or color (BGR) image containing (or not) QR codes.
		/// * points: Output vector of vector of vertices of the minimum-area quadrangle containing the codes.
		#[inline]
		fn detect_multi(&self, img: &impl ToInputArray, points: &mut impl ToOutputArray) -> Result<bool> {
			input_array_arg!(img);
			output_array_arg!(points);
			return_send!(via ocvrs_return);
			unsafe { sys::cv_QRCodeDetector_detectMulti_const_const__InputArrayR_const__OutputArrayR(self.as_raw_QRCodeDetector(), img.as_raw__InputArray(), points.as_raw__OutputArray(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		/// Decodes QR codes in image once it's found by the detect() method.
		/// ## Parameters
		/// * img: grayscale or color (BGR) image containing QR codes.
		/// * decoded_info: UTF8-encoded output vector of string or empty vector of string if the codes cannot be decoded.
		/// * points: vector of Quadrangle vertices found by detect() method (or some other algorithm).
		/// * straight_qrcode: The optional output vector of images containing rectified and binarized QR codes
		///
		/// ## C++ default parameters
		/// * straight_qrcode: noArray()
		#[inline]
		fn decode_multi(&self, img: &impl ToInputArray, points: &impl ToInputArray, decoded_info: &mut core::Vector<String>, straight_qrcode: &mut impl ToOutputArray) -> Result<bool> {
			input_array_arg!(img);
			input_array_arg!(points);
			output_array_arg!(straight_qrcode);
			return_send!(via ocvrs_return);
			unsafe { sys::cv_QRCodeDetector_decodeMulti_const_const__InputArrayR_const__InputArrayR_vectorLstringGR_const__OutputArrayR(self.as_raw_QRCodeDetector(), img.as_raw__InputArray(), points.as_raw__InputArray(), decoded_info.as_raw_mut_VectorOfString(), straight_qrcode.as_raw__OutputArray(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		/// Decodes QR codes in image once it's found by the detect() method.
		/// ## Parameters
		/// * img: grayscale or color (BGR) image containing QR codes.
		/// * decoded_info: UTF8-encoded output vector of string or empty vector of string if the codes cannot be decoded.
		/// * points: vector of Quadrangle vertices found by detect() method (or some other algorithm).
		/// * straight_qrcode: The optional output vector of images containing rectified and binarized QR codes
		///
		/// ## Note
		/// This alternative version of [QRCodeDetectorTraitConst::decode_multi] function uses the following default values for its arguments:
		/// * straight_qrcode: noArray()
		#[inline]
		fn decode_multi_def(&self, img: &impl ToInputArray, points: &impl ToInputArray, decoded_info: &mut core::Vector<String>) -> Result<bool> {
			input_array_arg!(img);
			input_array_arg!(points);
			return_send!(via ocvrs_return);
			unsafe { sys::cv_QRCodeDetector_decodeMulti_const_const__InputArrayR_const__InputArrayR_vectorLstringGR(self.as_raw_QRCodeDetector(), img.as_raw__InputArray(), points.as_raw__InputArray(), decoded_info.as_raw_mut_VectorOfString(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

	}

	/// Mutable methods for [crate::objdetect::QRCodeDetector]
	pub trait QRCodeDetectorTrait: crate::objdetect::QRCodeDetectorTraitConst {
		fn as_raw_mut_QRCodeDetector(&mut self) -> *mut c_void;

		/// sets the epsilon used during the horizontal scan of QR code stop marker detection.
		/// ## Parameters
		/// * epsX: Epsilon neighborhood, which allows you to determine the horizontal pattern
		/// of the scheme 1:1:3:1:1 according to QR code standard.
		#[inline]
		fn set_eps_x(&mut self, eps_x: f64) -> Result<()> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_QRCodeDetector_setEpsX_double(self.as_raw_mut_QRCodeDetector(), eps_x, ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		/// sets the epsilon used during the vertical scan of QR code stop marker detection.
		/// ## Parameters
		/// * epsY: Epsilon neighborhood, which allows you to determine the vertical pattern
		/// of the scheme 1:1:3:1:1 according to QR code standard.
		#[inline]
		fn set_eps_y(&mut self, eps_y: f64) -> Result<()> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_QRCodeDetector_setEpsY_double(self.as_raw_mut_QRCodeDetector(), eps_y, ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		/// Decodes QR code in image once it's found by the detect() method.
		///
		/// Returns UTF8-encoded output string or empty string if the code cannot be decoded.
		/// ## Parameters
		/// * img: grayscale or color (BGR) image containing QR code.
		/// * points: Quadrangle vertices found by detect() method (or some other algorithm).
		/// * straight_qrcode: The optional output image containing rectified and binarized QR code
		///
		/// ## C++ default parameters
		/// * straight_qrcode: noArray()
		#[inline]
		fn decode(&mut self, img: &impl ToInputArray, points: &impl ToInputArray, straight_qrcode: &mut impl ToOutputArray) -> Result<Vec<u8>> {
			input_array_arg!(img);
			input_array_arg!(points);
			output_array_arg!(straight_qrcode);
			return_send!(via ocvrs_return);
			unsafe { sys::cv_QRCodeDetector_decode_const__InputArrayR_const__InputArrayR_const__OutputArrayR(self.as_raw_mut_QRCodeDetector(), img.as_raw__InputArray(), points.as_raw__InputArray(), straight_qrcode.as_raw__OutputArray(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			let ret = unsafe { Vec::<u8>::opencv_from_extern(ret) };
			Ok(ret)
		}

		/// Decodes QR code in image once it's found by the detect() method.
		///
		/// Returns UTF8-encoded output string or empty string if the code cannot be decoded.
		/// ## Parameters
		/// * img: grayscale or color (BGR) image containing QR code.
		/// * points: Quadrangle vertices found by detect() method (or some other algorithm).
		/// * straight_qrcode: The optional output image containing rectified and binarized QR code
		///
		/// ## Note
		/// This alternative version of [QRCodeDetectorTrait::decode] function uses the following default values for its arguments:
		/// * straight_qrcode: noArray()
		#[inline]
		fn decode_def(&mut self, img: &impl ToInputArray, points: &impl ToInputArray) -> Result<Vec<u8>> {
			input_array_arg!(img);
			input_array_arg!(points);
			return_send!(via ocvrs_return);
			unsafe { sys::cv_QRCodeDetector_decode_const__InputArrayR_const__InputArrayR(self.as_raw_mut_QRCodeDetector(), img.as_raw__InputArray(), points.as_raw__InputArray(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			let ret = unsafe { Vec::<u8>::opencv_from_extern(ret) };
			Ok(ret)
		}

		/// Decodes QR code on a curved surface in image once it's found by the detect() method.
		///
		/// Returns UTF8-encoded output string or empty string if the code cannot be decoded.
		/// ## Parameters
		/// * img: grayscale or color (BGR) image containing QR code.
		/// * points: Quadrangle vertices found by detect() method (or some other algorithm).
		/// * straight_qrcode: The optional output image containing rectified and binarized QR code
		///
		/// ## C++ default parameters
		/// * straight_qrcode: noArray()
		#[inline]
		fn decode_curved(&mut self, img: &impl ToInputArray, points: &impl ToInputArray, straight_qrcode: &mut impl ToOutputArray) -> Result<Vec<u8>> {
			input_array_arg!(img);
			input_array_arg!(points);
			output_array_arg!(straight_qrcode);
			return_send!(via ocvrs_return);
			unsafe { sys::cv_QRCodeDetector_decodeCurved_const__InputArrayR_const__InputArrayR_const__OutputArrayR(self.as_raw_mut_QRCodeDetector(), img.as_raw__InputArray(), points.as_raw__InputArray(), straight_qrcode.as_raw__OutputArray(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			let ret = unsafe { Vec::<u8>::opencv_from_extern(ret) };
			Ok(ret)
		}

		/// Decodes QR code on a curved surface in image once it's found by the detect() method.
		///
		/// Returns UTF8-encoded output string or empty string if the code cannot be decoded.
		/// ## Parameters
		/// * img: grayscale or color (BGR) image containing QR code.
		/// * points: Quadrangle vertices found by detect() method (or some other algorithm).
		/// * straight_qrcode: The optional output image containing rectified and binarized QR code
		///
		/// ## Note
		/// This alternative version of [QRCodeDetectorTrait::decode_curved] function uses the following default values for its arguments:
		/// * straight_qrcode: noArray()
		#[inline]
		fn decode_curved_def(&mut self, img: &impl ToInputArray, points: &impl ToInputArray) -> Result<Vec<u8>> {
			input_array_arg!(img);
			input_array_arg!(points);
			return_send!(via ocvrs_return);
			unsafe { sys::cv_QRCodeDetector_decodeCurved_const__InputArrayR_const__InputArrayR(self.as_raw_mut_QRCodeDetector(), img.as_raw__InputArray(), points.as_raw__InputArray(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			let ret = unsafe { Vec::<u8>::opencv_from_extern(ret) };
			Ok(ret)
		}

		/// Both detects and decodes QR code
		///
		/// ## Parameters
		/// * img: grayscale or color (BGR) image containing QR code.
		/// * points: optional output array of vertices of the found QR code quadrangle. Will be empty if not found.
		/// * straight_qrcode: The optional output image containing rectified and binarized QR code
		///
		/// ## C++ default parameters
		/// * points: noArray()
		/// * straight_qrcode: noArray()
		#[inline]
		fn detect_and_decode(&mut self, img: &impl ToInputArray, points: &mut impl ToOutputArray, straight_qrcode: &mut impl ToOutputArray) -> Result<Vec<u8>> {
			input_array_arg!(img);
			output_array_arg!(points);
			output_array_arg!(straight_qrcode);
			return_send!(via ocvrs_return);
			unsafe { sys::cv_QRCodeDetector_detectAndDecode_const__InputArrayR_const__OutputArrayR_const__OutputArrayR(self.as_raw_mut_QRCodeDetector(), img.as_raw__InputArray(), points.as_raw__OutputArray(), straight_qrcode.as_raw__OutputArray(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			let ret = unsafe { Vec::<u8>::opencv_from_extern(ret) };
			Ok(ret)
		}

		/// Both detects and decodes QR code
		///
		/// ## Parameters
		/// * img: grayscale or color (BGR) image containing QR code.
		/// * points: optional output array of vertices of the found QR code quadrangle. Will be empty if not found.
		/// * straight_qrcode: The optional output image containing rectified and binarized QR code
		///
		/// ## Note
		/// This alternative version of [QRCodeDetectorTrait::detect_and_decode] function uses the following default values for its arguments:
		/// * points: noArray()
		/// * straight_qrcode: noArray()
		#[inline]
		fn detect_and_decode_def(&mut self, img: &impl ToInputArray) -> Result<Vec<u8>> {
			input_array_arg!(img);
			return_send!(via ocvrs_return);
			unsafe { sys::cv_QRCodeDetector_detectAndDecode_const__InputArrayR(self.as_raw_mut_QRCodeDetector(), img.as_raw__InputArray(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			let ret = unsafe { Vec::<u8>::opencv_from_extern(ret) };
			Ok(ret)
		}

		/// Both detects and decodes QR code on a curved surface
		///
		/// ## Parameters
		/// * img: grayscale or color (BGR) image containing QR code.
		/// * points: optional output array of vertices of the found QR code quadrangle. Will be empty if not found.
		/// * straight_qrcode: The optional output image containing rectified and binarized QR code
		///
		/// ## C++ default parameters
		/// * points: noArray()
		/// * straight_qrcode: noArray()
		#[inline]
		fn detect_and_decode_curved(&mut self, img: &impl ToInputArray, points: &mut impl ToOutputArray, straight_qrcode: &mut impl ToOutputArray) -> Result<Vec<u8>> {
			input_array_arg!(img);
			output_array_arg!(points);
			output_array_arg!(straight_qrcode);
			return_send!(via ocvrs_return);
			unsafe { sys::cv_QRCodeDetector_detectAndDecodeCurved_const__InputArrayR_const__OutputArrayR_const__OutputArrayR(self.as_raw_mut_QRCodeDetector(), img.as_raw__InputArray(), points.as_raw__OutputArray(), straight_qrcode.as_raw__OutputArray(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			let ret = unsafe { Vec::<u8>::opencv_from_extern(ret) };
			Ok(ret)
		}

		/// Both detects and decodes QR code on a curved surface
		///
		/// ## Parameters
		/// * img: grayscale or color (BGR) image containing QR code.
		/// * points: optional output array of vertices of the found QR code quadrangle. Will be empty if not found.
		/// * straight_qrcode: The optional output image containing rectified and binarized QR code
		///
		/// ## Note
		/// This alternative version of [QRCodeDetectorTrait::detect_and_decode_curved] function uses the following default values for its arguments:
		/// * points: noArray()
		/// * straight_qrcode: noArray()
		#[inline]
		fn detect_and_decode_curved_def(&mut self, img: &impl ToInputArray) -> Result<Vec<u8>> {
			input_array_arg!(img);
			return_send!(via ocvrs_return);
			unsafe { sys::cv_QRCodeDetector_detectAndDecodeCurved_const__InputArrayR(self.as_raw_mut_QRCodeDetector(), img.as_raw__InputArray(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			let ret = unsafe { Vec::<u8>::opencv_from_extern(ret) };
			Ok(ret)
		}

	}

	impl std::fmt::Debug for QRCodeDetector {
		#[inline]
		fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
			f.debug_struct("QRCodeDetector")
				.finish()
		}
	}

	impl crate::objdetect::QRCodeDetectorTraitConst for QRCodeDetector {
		#[inline] fn as_raw_QRCodeDetector(&self) -> *const c_void { self.as_raw() }
	}

	impl crate::objdetect::QRCodeDetectorTrait for QRCodeDetector {
		#[inline] fn as_raw_mut_QRCodeDetector(&mut self) -> *mut c_void { self.as_raw_mut() }
	}

	boxed_ref! { QRCodeDetector, crate::objdetect::QRCodeDetectorTraitConst, as_raw_QRCodeDetector, crate::objdetect::QRCodeDetectorTrait, as_raw_mut_QRCodeDetector }

	pub struct QRCodeEncoder {
		ptr: *mut c_void,
	}

	opencv_type_boxed! { QRCodeEncoder }

	impl Drop for QRCodeEncoder {
		#[inline]
		fn drop(&mut self) {
			unsafe { sys::cv_QRCodeEncoder_delete(self.as_raw_mut_QRCodeEncoder()) };
		}
	}

	unsafe impl Send for QRCodeEncoder {}

	impl QRCodeEncoder {
		/// Constructor
		/// ## Parameters
		/// * parameters: QR code encoder parameters QRCodeEncoder::Params
		///
		/// ## C++ default parameters
		/// * parameters: QRCodeEncoder::Params()
		#[inline]
		pub fn create(parameters: crate::objdetect::QRCodeEncoder_Params) -> Result<core::Ptr<crate::objdetect::QRCodeEncoder>> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_QRCodeEncoder_create_const_ParamsR(&parameters, ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			let ret = unsafe { core::Ptr::<crate::objdetect::QRCodeEncoder>::opencv_from_extern(ret) };
			Ok(ret)
		}

		/// Constructor
		/// ## Parameters
		/// * parameters: QR code encoder parameters QRCodeEncoder::Params
		///
		/// ## Note
		/// This alternative version of [QRCodeEncoder::create] function uses the following default values for its arguments:
		/// * parameters: QRCodeEncoder::Params()
		#[inline]
		pub fn create_def() -> Result<core::Ptr<crate::objdetect::QRCodeEncoder>> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_QRCodeEncoder_create(ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			let ret = unsafe { core::Ptr::<crate::objdetect::QRCodeEncoder>::opencv_from_extern(ret) };
			Ok(ret)
		}

	}

	/// Constant methods for [crate::objdetect::QRCodeEncoder]
	pub trait QRCodeEncoderTraitConst {
		fn as_raw_QRCodeEncoder(&self) -> *const c_void;

	}

	/// Mutable methods for [crate::objdetect::QRCodeEncoder]
	pub trait QRCodeEncoderTrait: crate::objdetect::QRCodeEncoderTraitConst {
		fn as_raw_mut_QRCodeEncoder(&mut self) -> *mut c_void;

		/// Generates QR code from input string.
		/// ## Parameters
		/// * encoded_info: Input string to encode.
		/// * qrcode: Generated QR code.
		#[inline]
		fn encode(&mut self, encoded_info: &str, qrcode: &mut impl ToOutputArray) -> Result<()> {
			extern_container_arg!(encoded_info);
			output_array_arg!(qrcode);
			return_send!(via ocvrs_return);
			unsafe { sys::cv_QRCodeEncoder_encode_const_StringR_const__OutputArrayR(self.as_raw_mut_QRCodeEncoder(), encoded_info.opencv_as_extern(), qrcode.as_raw__OutputArray(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

		/// Generates QR code from input string in Structured Append mode. The encoded message is splitting over a number of QR codes.
		/// ## Parameters
		/// * encoded_info: Input string to encode.
		/// * qrcodes: Vector of generated QR codes.
		#[inline]
		fn encode_structured_append(&mut self, encoded_info: &str, qrcodes: &mut impl ToOutputArray) -> Result<()> {
			extern_container_arg!(encoded_info);
			output_array_arg!(qrcodes);
			return_send!(via ocvrs_return);
			unsafe { sys::cv_QRCodeEncoder_encodeStructuredAppend_const_StringR_const__OutputArrayR(self.as_raw_mut_QRCodeEncoder(), encoded_info.opencv_as_extern(), qrcodes.as_raw__OutputArray(), ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

	}

	impl std::fmt::Debug for QRCodeEncoder {
		#[inline]
		fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
			f.debug_struct("QRCodeEncoder")
				.finish()
		}
	}

	impl crate::objdetect::QRCodeEncoderTraitConst for QRCodeEncoder {
		#[inline] fn as_raw_QRCodeEncoder(&self) -> *const c_void { self.as_raw() }
	}

	impl crate::objdetect::QRCodeEncoderTrait for QRCodeEncoder {
		#[inline] fn as_raw_mut_QRCodeEncoder(&mut self) -> *mut c_void { self.as_raw_mut() }
	}

	boxed_ref! { QRCodeEncoder, crate::objdetect::QRCodeEncoderTraitConst, as_raw_QRCodeEncoder, crate::objdetect::QRCodeEncoderTrait, as_raw_mut_QRCodeEncoder }

	/// QR code encoder parameters.
	/// ## Parameters
	/// * version: The optional version of QR code (by default - maximum possible depending on
	///                the length of the string).
	/// * correction_level: The optional level of error correction (by default - the lowest).
	/// * mode: The optional encoding mode - Numeric, Alphanumeric, Byte, Kanji, ECI or Structured Append.
	/// * structure_number: The optional number of QR codes to generate in Structured Append mode.
	#[repr(C)]
	#[derive(Copy, Clone, Debug, PartialEq)]
	pub struct QRCodeEncoder_Params {
		pub version: i32,
		pub correction_level: crate::objdetect::QRCodeEncoder_CorrectionLevel,
		pub mode: crate::objdetect::QRCodeEncoder_EncodeMode,
		pub structure_number: i32,
	}

	opencv_type_simple! { crate::objdetect::QRCodeEncoder_Params }

	impl QRCodeEncoder_Params {
		#[inline]
		pub fn default() -> Result<crate::objdetect::QRCodeEncoder_Params> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_QRCodeEncoder_Params_Params(ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

	}

	/// This class is used for grouping object candidates detected by Cascade Classifier, HOG etc.
	///
	/// instance of the class is to be passed to cv::partition
	pub struct SimilarRects {
		ptr: *mut c_void,
	}

	opencv_type_boxed! { SimilarRects }

	impl Drop for SimilarRects {
		#[inline]
		fn drop(&mut self) {
			unsafe { sys::cv_SimilarRects_delete(self.as_raw_mut_SimilarRects()) };
		}
	}

	unsafe impl Send for SimilarRects {}

	impl SimilarRects {
		#[inline]
		pub fn new(_eps: f64) -> Result<crate::objdetect::SimilarRects> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_SimilarRects_SimilarRects_double(_eps, ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			let ret = unsafe { crate::objdetect::SimilarRects::opencv_from_extern(ret) };
			Ok(ret)
		}

	}

	/// Constant methods for [crate::objdetect::SimilarRects]
	pub trait SimilarRectsTraitConst {
		fn as_raw_SimilarRects(&self) -> *const c_void;

		#[inline]
		fn eps(&self) -> f64 {
			let ret = unsafe { sys::cv_SimilarRects_propEps_const(self.as_raw_SimilarRects()) };
			ret
		}

		#[inline]
		fn apply(&self, r1: core::Rect, r2: core::Rect) -> Result<bool> {
			return_send!(via ocvrs_return);
			unsafe { sys::cv_SimilarRects_operator___const_const_RectR_const_RectR(self.as_raw_SimilarRects(), &r1, &r2, ocvrs_return.as_mut_ptr()) };
			return_receive!(ocvrs_return => ret);
			let ret = ret.into_result()?;
			Ok(ret)
		}

	}

	/// Mutable methods for [crate::objdetect::SimilarRects]
	pub trait SimilarRectsTrait: crate::objdetect::SimilarRectsTraitConst {
		fn as_raw_mut_SimilarRects(&mut self) -> *mut c_void;

		#[inline]
		fn set_eps(&mut self, val: f64) {
			let ret = unsafe { sys::cv_SimilarRects_propEps_const_double(self.as_raw_mut_SimilarRects(), val) };
			ret
		}

	}

	impl std::fmt::Debug for SimilarRects {
		#[inline]
		fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
			f.debug_struct("SimilarRects")
				.field("eps", &crate::objdetect::SimilarRectsTraitConst::eps(self))
				.finish()
		}
	}

	impl crate::objdetect::SimilarRectsTraitConst for SimilarRects {
		#[inline] fn as_raw_SimilarRects(&self) -> *const c_void { self.as_raw() }
	}

	impl crate::objdetect::SimilarRectsTrait for SimilarRects {
		#[inline] fn as_raw_mut_SimilarRects(&mut self) -> *mut c_void { self.as_raw_mut() }
	}

	boxed_ref! { SimilarRects, crate::objdetect::SimilarRectsTraitConst, as_raw_SimilarRects, crate::objdetect::SimilarRectsTrait, as_raw_mut_SimilarRects }

}
