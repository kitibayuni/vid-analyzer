#include "ocvrs_common.hpp"
#include <opencv2/cvv.hpp>
#include "cvv_types.hpp"

extern "C" {
void cvv_impl_debugDMatch_const__InputArrayR_vectorLKeyPointG_const__InputArrayR_vectorLKeyPointG_vectorLDMatchG_const_CallMetaDataR_const_charX_const_charX_bool(const cv::_InputArray* img1, std::vector<cv::KeyPoint>* keypoints1, const cv::_InputArray* img2, std::vector<cv::KeyPoint>* keypoints2, std::vector<cv::DMatch>* matches, const cvv::impl::CallMetaData* data, const char* description, const char* view, bool useTrainDescriptor, ResultVoid* ocvrs_return) {
	try {
		cvv::impl::debugDMatch(*img1, *keypoints1, *img2, *keypoints2, *matches, *data, description, view, useTrainDescriptor);
		Ok(ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cvv_impl_debugFilter_const__InputArrayR_const__InputArrayR_const_CallMetaDataR_const_charX_const_charX(const cv::_InputArray* original, const cv::_InputArray* result, const cvv::impl::CallMetaData* data, const char* description, const char* view, ResultVoid* ocvrs_return) {
	try {
		cvv::impl::debugFilter(*original, *result, *data, description, view);
		Ok(ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cvv_impl_finalShow(ResultVoid* ocvrs_return) {
	try {
		cvv::impl::finalShow();
		Ok(ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cvv_impl_showImage_const__InputArrayR_const_CallMetaDataR_const_charX_const_charX(const cv::_InputArray* img, const cvv::impl::CallMetaData* data, const char* description, const char* view, ResultVoid* ocvrs_return) {
	try {
		cvv::impl::showImage(*img, *data, description, view);
		Ok(ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cvv_impl_CallMetaData_CallMetaData(Result<cvv::impl::CallMetaData*>* ocvrs_return) {
	try {
		cvv::impl::CallMetaData* ret = new cvv::impl::CallMetaData();
		Ok(ret, ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cvv_impl_CallMetaData_CallMetaData_const_charX_size_t_const_charX(const char* file, size_t line, const char* function, Result<cvv::impl::CallMetaData*>* ocvrs_return) {
	try {
		cvv::impl::CallMetaData* ret = new cvv::impl::CallMetaData(file, line, function);
		Ok(ret, ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

void cvv_impl_CallMetaData_operator_bool(cvv::impl::CallMetaData* instance, Result<bool>* ocvrs_return) {
	try {
		bool ret = instance->operator bool();
		Ok(ret, ocvrs_return);
	} OCVRS_CATCH(ocvrs_return);
}

cvv::impl::CallMetaData* cvv_impl_CallMetaData_implicitClone_const(const cvv::impl::CallMetaData* instance) {
		return new cvv::impl::CallMetaData(*instance);
}

void* cvv_impl_CallMetaData_propFile_const(const cvv::impl::CallMetaData* instance) {
		const char* ret = instance->file;
		return ocvrs_create_string(ret);
}

const size_t cvv_impl_CallMetaData_propLine_const(const cvv::impl::CallMetaData* instance) {
		const size_t ret = instance->line;
		return ret;
}

void* cvv_impl_CallMetaData_propFunction_const(const cvv::impl::CallMetaData* instance) {
		const char* ret = instance->function;
		return ocvrs_create_string(ret);
}

const bool cvv_impl_CallMetaData_propIsKnown_const(const cvv::impl::CallMetaData* instance) {
		const bool ret = instance->isKnown;
		return ret;
}

void cvv_impl_CallMetaData_delete(cvv::impl::CallMetaData* instance) {
		delete instance;
}

}
